use crate::config::Config;
use crate::custom_errors::NodeCustomErrors;
use crate::logwriter::log_writer::{write_in_log, LogSender};
use crate::messages::message_header::{
    read_verack_message, write_sendheaders_message, write_verack_message,
};
use crate::messages::version_message::{get_version_message, VersionMessage};
use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::result::Result;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

/// Realiza la conexión a los nodos con múltiples threads
/// Recibe las direcciones IP de los nodos.
/// Devuelve un vector de sockets o un error si no se pudo completar.
pub fn handshake_with_nodes(
    config: &Arc<Config>,
    log_sender: &LogSender,
    node_ips: Vec<Ipv4Addr>,
) -> Result<Arc<RwLock<Vec<TcpStream>>>, NodeCustomErrors> {
    write_in_log(&log_sender.info_log_sender, "INICIO DE HANDSHAKE");
    println!("Realizando handshake con los nodos...");
    let chunk_size = (node_ips.len() as f64 / config.n_threads as f64).ceil() as usize;
    let active_nodes_chunks = Arc::new(RwLock::new(
        node_ips
            .chunks(chunk_size)
            .map(|chunk| chunk.to_vec())
            .collect::<Vec<_>>(),
    ));
    let sockets = vec![];
    let sockets_lock = Arc::new(RwLock::new(sockets));
    let mut thread_handles = vec![];
    for i in 0..config.n_threads {
        if i >= active_nodes_chunks
            .read()
            .map_err(|err| NodeCustomErrors::LockError(format!("{}", err)))?
            .len()
        {
            break;
        }
        let chunk = active_nodes_chunks
            .write()
            .map_err(|err| NodeCustomErrors::LockError(format!("{}", err)))?[i]
            .clone();
        let config = config.clone();
        let log_sender_clone = log_sender.clone();
        let sockets: Arc<RwLock<Vec<TcpStream>>> = Arc::clone(&sockets_lock);
        thread_handles.push(thread::spawn(move || {
            connect_to_nodes(&config, &log_sender_clone, sockets, &chunk)
        }));
    }
    for handle in thread_handles {
        handle
            .join()
            .map_err(|err| NodeCustomErrors::ThreadJoinError(format!("{:?}", err)))??;
    }
    let amount_of_ips = sockets_lock
        .read()
        .map_err(|err| NodeCustomErrors::LockError(format!("{:?}", err)))?
        .len();
    write_in_log(
        &log_sender.info_log_sender,
        format!("{:?} nodos conectados", amount_of_ips).as_str(),
    );
    write_in_log(
        &log_sender.info_log_sender,
        "Se completo correctamente el handshake\n",
    );
    Ok(sockets_lock)
}

/// Realiza la conexión con todos los nodos de la lista recibida por parámetro.
/// Guarda el los mismos en la lista de sockets recibida.
/// En caso de no poder conectarse, continua intentando con el siguiente.
fn connect_to_nodes(
    config: &Arc<Config>,
    log_sender: &LogSender,
    sockets: Arc<RwLock<Vec<TcpStream>>>,
    nodes: &[Ipv4Addr],
) -> Result<(), NodeCustomErrors> {
    for node in nodes {
        match connect_to_node(config, log_sender, node) {
            Ok(stream) => {
                write_in_log(
                    &log_sender.info_log_sender,
                    format!("Conectado correctamente a: {:?}", node).as_str(),
                );
                sockets
                    .write()
                    .map_err(|err| NodeCustomErrors::LockError(format!("{}", err)))?
                    .push(stream);
            }
            Err(err) => {
                write_in_log(
                    &log_sender.error_log_sender,
                    format!("No se pudo conectar al nodo: {:?}. Error {:?}.", node, err).as_str(),
                );
            }
        };
    }
    // si no se pudo conectar a ningun nodo devuelvo error
    if sockets
        .read()
        .map_err(|err| NodeCustomErrors::LockError(format!("{}", err)))?
        .is_empty()
    {
        return Err(NodeCustomErrors::HandshakeError(
            "No se pudo conectar a ningun nodo".to_string(),
        ));
    }
    Ok(())
}

/// Realiza la conexión con un nodo.
/// Envía y recibe los mensajes necesarios para establecer la conexión
/// Devuelve el socket o un error
fn connect_to_node(
    config: &Arc<Config>,
    log_sender: &LogSender,
    node_ip: &Ipv4Addr,
) -> Result<TcpStream, Box<dyn Error>> {
    let socket_addr = SocketAddr::new((*node_ip).into(), config.net_port);
    let mut stream: TcpStream =
        TcpStream::connect_timeout(&socket_addr, Duration::from_secs(config.connect_timeout))?;
    let local_ip_addr = stream.local_addr()?;
    let version_message = get_version_message(config, socket_addr, local_ip_addr)?;
    version_message.write_to(&mut stream)?;
    VersionMessage::read_from(log_sender, &mut stream)?;
    write_verack_message(&mut stream)?;
    read_verack_message(log_sender, &mut stream)?;
    write_sendheaders_message(&mut stream)?;
    Ok(stream)
}
