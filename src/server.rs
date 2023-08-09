use std::{
    net::{IpAddr, SocketAddr, TcpListener, TcpStream},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread::{spawn, JoinHandle},
};

use gtk::glib;

use crate::{
    config::Config,
    custom_errors::NodeCustomErrors,
    gtk::ui_events::UIEvent,
    logwriter::log_writer::{write_in_log, LogSender},
    messages::{
        message_header::{read_verack_message, write_verack_message},
        version_message::{get_version_message, VersionMessage},
    },
    node::Node,
};

const LOCALHOST: &str = "127.0.0.1";

#[derive(Debug)]
/// Estructura que representa al servidor de un nodo.
/// Sender para indicarle al TcpListener que deje de escuchar por conexiones entrantes
/// handle para esperar oportunamente al thread que esucha conexiones entrantes
pub struct NodeServer {
    sender: Sender<String>,
    handle: JoinHandle<Result<(), NodeCustomErrors>>,
}

impl NodeServer {
    /// Crea un nuevo servidor de nodo en un thread aparte encargado de eso

    pub fn new(
        config: &Arc<Config>,
        log_sender: &LogSender,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        node: &mut Node,
    ) -> Result<NodeServer, NodeCustomErrors> {
        let (sender, rx) = mpsc::channel();
        let address = get_socket(LOCALHOST.to_string(), config.net_port)?;
        let mut node_clone = node.clone();
        let log_sender_clone = log_sender.clone();
        let config = config.clone();
        let ui_sender = ui_sender.clone();
        let handle = spawn(move || {
            Self::listen(
                &config,
                &log_sender_clone,
                &ui_sender,
                &mut node_clone,
                address,
                rx,
            )
        });
        Ok(NodeServer { sender, handle })
    }

    /// Escucha por conexiones entrantes y las maneja
    /// Si llega un mensaje por el channel, sigifica que debe dejar de escuchar y cortar el bucle
    /// Devuelve un error si ocurre alguno que no sea del tipo WouldBlock
    fn listen(
        config: &Arc<Config>,
        log_sender: &LogSender,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        node: &mut Node,
        address: SocketAddr,
        rx: Receiver<String>,
    ) -> Result<(), NodeCustomErrors> {
        let address = format!("{}:{}", address.ip(), address.port());
        let listener: TcpListener = TcpListener::bind(&address)
            .map_err(|err| NodeCustomErrors::SocketError(err.to_string()))?;
        listener
            .set_nonblocking(true)
            .map_err(|err| NodeCustomErrors::SocketError(err.to_string()))?;
        let amount_of_connections = 0;
        write_in_log(
            &log_sender.info_log_sender,
            "Empiezo a escuchar por conecciones entrantes!",
        );
        for stream in listener.incoming() {
            // recibio un mensaje para frenar
            if rx.try_recv().is_ok() {
                write_in_log(
                    &log_sender.info_log_sender,
                    "Dejo de escuchar por conexiones entrantes!",
                );
                break;
            }
            match stream {
                Ok(stream) => {
                    if amount_of_connections > config.max_connections_to_server {
                        break;
                    }
                    write_in_log(
                        &log_sender.info_log_sender,
                        format!(
                            "Recibo nueva conexion entrante --{:?}--",
                            stream.peer_addr()
                        )
                        .as_str(),
                    );
                    Self::handle_incoming_connection(config, log_sender, ui_sender, node, stream)?;
                }
                Err(ref err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    // This doesen't mean an error ocurred, there just wasn't a connection at the moment
                    continue;
                }
                Err(err) => return Err(NodeCustomErrors::CanNotRead(err.to_string())),
            }
        }
        Ok(())
    }
    /// Maneja una conexion entrante
    /// Realiza el handshake y agrega la conexion al nodo
    /// Devuelve un error si ocurre alguno
    fn handle_incoming_connection(
        config: &Arc<Config>,
        log_sender: &LogSender,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        node: &mut Node,
        mut stream: TcpStream,
    ) -> Result<(), NodeCustomErrors> {
        // REALIZAR EL HANDSHAKE
        let local_ip_addr = stream
            .local_addr()
            .map_err(|err| NodeCustomErrors::SocketError(err.to_string()))?;
        let socket_addr = stream
            .peer_addr()
            .map_err(|err| NodeCustomErrors::SocketError(err.to_string()))?;
        VersionMessage::read_from(log_sender, &mut stream)
            .map_err(|err| NodeCustomErrors::CanNotRead(err.to_string()))?;
        let version_message = get_version_message(config, socket_addr, local_ip_addr)
            .map_err(|err| NodeCustomErrors::OtherError(err.to_string()))?;
        version_message
            .write_to(&mut stream)
            .map_err(|err| NodeCustomErrors::WriteNodeError(err.to_string()))?;
        read_verack_message(log_sender, &mut stream)
            .map_err(|err| NodeCustomErrors::CanNotRead(err.to_string()))?;
        write_verack_message(&mut stream)
            .map_err(|err| NodeCustomErrors::WriteNodeError(err.to_string()))?;
        write_in_log(
            &log_sender.info_log_sender,
            format!("Handshake con nodo {:?} realizado con exito!", socket_addr).as_str(),
        );
        // AGREGAR LA CONEXION AL NODO
        node.add_connection(log_sender, ui_sender, stream)?;
        Ok(())
    }

    /// Le indica al servidor que deje de escuchar por conexiones entrantes
    /// Envia por el channel un string (puede ser cualquiera) y le idica al thread que deje de escuchar en el bucle
    pub fn shutdown_server(self) -> Result<(), NodeCustomErrors> {
        self.sender
            .send("finish".to_string())
            .map_err(|err| NodeCustomErrors::ThreadChannelError(err.to_string()))?;
        self.handle.join().map_err(|_| {
            NodeCustomErrors::ThreadJoinError(
                "Error al hacer join al thread del servidor que esucha".to_string(),
            )
        })??;
        Ok(())
    }
}

/// Devuelve un SocketAddr a partir de una ip y un puerto
fn get_socket(ip: String, port: u16) -> Result<SocketAddr, NodeCustomErrors> {
    let ip = ip
        .parse::<IpAddr>()
        .map_err(|err| NodeCustomErrors::SocketError(err.to_string()))?;
    Ok(SocketAddr::new(ip, port))
}
