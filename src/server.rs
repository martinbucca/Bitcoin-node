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
/// Represents a node server.
/// Sender to indicate to the TcpListener to stop listening for incoming connections
/// handle to wait for the thread that listens for incoming connections
pub struct NodeServer {
    sender: Sender<String>,
    handle: JoinHandle<Result<(), NodeCustomErrors>>,
}

impl NodeServer {
    /// Creates a new NodeServer
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

    /// Listen for incoming connections and handles them.
    /// If a message arrives by the channel, it means that it must stop listening and cut the loop.
    /// Returns an error if any occurs that is not of the type WouldBlock.
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
        let mut amount_of_connections = 0;
        write_in_log(
            &log_sender.info_log_sender,
            "Start listening for incoming connections!",
        );
        for stream in listener.incoming() {
            // stop message
            if rx.try_recv().is_ok() {
                write_in_log(
                    &log_sender.info_log_sender,
                    "Stop listening for incoming connections!",
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
                            "Receives new incoming connection from --{:?}--",
                            stream.peer_addr()
                        )
                        .as_str(),
                    );
                    Self::handle_incoming_connection(config, log_sender, ui_sender, node, stream)?;
                    amount_of_connections += 1;
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

    /// Handles an incoming connection.
    /// Performs the handshake and adds the connection to the node.
    /// Returns an error if any occurs.
    fn handle_incoming_connection(
        config: &Arc<Config>,
        log_sender: &LogSender,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        node: &mut Node,
        mut stream: TcpStream,
    ) -> Result<(), NodeCustomErrors> {
        // HANDSHAKE
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
            format!("Handshake with node --{:?}-- done successfully!", socket_addr).as_str(),
        );
        // ADD CONNECTION TO NODE
        node.add_connection(log_sender, ui_sender, stream)?;
        Ok(())
    }

    /// Indicates to the server to stop listening for incoming connections.
    /// Sends a string (can be anything) through the channel and tells the thread to stop listening in the loop
    /// and to join the thread.
    /// Returns an error if it can't send the message through the channel or if it can't join the thread.
    pub fn shutdown_server(self) -> Result<(), NodeCustomErrors> {
        self.sender
            .send("finish".to_string())
            .map_err(|err| NodeCustomErrors::ThreadChannelError(err.to_string()))?;
        self.handle.join().map_err(|_| {
            NodeCustomErrors::ThreadJoinError(
                "Error trying to join the thread that listens for incoming connections!".to_string(),
            )
        })??;
        Ok(())
    }
}

/// Returns a SocketAddr from an ip and a port
fn get_socket(ip: String, port: u16) -> Result<SocketAddr, NodeCustomErrors> {
    let ip = ip
        .parse::<IpAddr>()
        .map_err(|err| NodeCustomErrors::SocketError(err.to_string()))?;
    Ok(SocketAddr::new(ip, port))
}
