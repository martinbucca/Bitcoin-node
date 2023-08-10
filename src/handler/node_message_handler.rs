use gtk::glib;

use crate::{
    custom_errors::NodeCustomErrors,
    gtk::ui_events::UIEvent,
    logwriter::log_writer::{write_in_log, LogSender},
    messages::{message_header::is_terminated, message_header::HeaderMessage},
    node_data_pointers::NodeDataPointers,
};
use std::{
    io::{self, Read, Write},
    mem,
    net::TcpStream,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex, RwLock,
    },
    thread::{self, JoinHandle},
};

use super::message_handlers::{
    handle_block_message, handle_getdata_message, handle_getheaders_message,
    handle_headers_message, handle_inv_message, handle_ping_message, handle_tx_message,
    write_to_node,
};

type NodeMessageHandlerResult = Result<(), NodeCustomErrors>;
type NodeSender = Sender<Vec<u8>>;
type NodeReceiver = Receiver<Vec<u8>>;

#[derive(Debug, Clone)]
/// Struct to control all the nodes connected to ours. It listens permanently
/// to these and decides what to do with the messages that arrive and with those that it has to write.
pub struct NodeMessageHandler {
    nodes_handle: Arc<Mutex<Vec<JoinHandle<()>>>>,
    nodes_sender: Vec<NodeSender>, // Stores all the sender to write to the nodes
    transactions_recieved: Arc<RwLock<Vec<[u8; 32]>>>,
    finish: Arc<RwLock<bool>>,
}

impl NodeMessageHandler {
    /// Receives the information that the node has (headers, blocks and connected nodes)
    /// and is responsible for creating a thread for each node and leaving it listening to messages
    /// and handling them in a timely manner. If an error occurs, it returns an Error of the enum
    /// NodeCustomErrors and otherwise returns the new struct.
    pub fn new(
        log_sender: &LogSender,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        node_pointers: NodeDataPointers,
    ) -> Result<Self, NodeCustomErrors> {
        write_in_log(
            &log_sender.info_log_sender,
            "Starting to listen to nodes...\n",
        );
        let finish = Arc::new(RwLock::new(false));
        let mut nodes_handle: Vec<JoinHandle<()>> = vec![];
        let amount_nodes = get_amount_of_nodes(node_pointers.connected_nodes.clone())?;
        let mut nodes_sender = vec![];
        // list of received transactions to not receive the same from several nodes
        let transactions_recieved: Arc<RwLock<Vec<[u8; 32]>>> = Arc::new(RwLock::new(Vec::new()));
        for _ in 0..amount_nodes {
            let (tx, rx) = channel();
            nodes_sender.push(tx.clone());
            let node = get_last_node(node_pointers.connected_nodes.clone())?;
            println!(
                "Node -{:?}- Listening for new blocks and transactions...\n",
                node.peer_addr()
            );
            nodes_handle.push(handle_messages_from_node(
                log_sender,
                ui_sender,
                (tx, rx),
                transactions_recieved.clone(),
                node_pointers.clone(),
                node,
                Some(finish.clone()),
            ))
        }
        let nodes_handle_mutex = Arc::new(Mutex::new(nodes_handle));
        Ok(NodeMessageHandler {
            nodes_handle: nodes_handle_mutex,
            nodes_sender,
            transactions_recieved,
            finish,
        })
    }

    /// Receives a vector of bytes that represents a serialized message and sends it to each channel that is waiting to write to a node.
    /// In this way the message is broadcast to all connected nodes.
    /// Returns Ok(()) in case of success or a ThreadChannelError error otherwise.
    pub fn broadcast_to_nodes(&self, message: Vec<u8>) -> NodeMessageHandlerResult {
        let mut amount_of_failed_nodes = 0;
        for node_sender in &self.nodes_sender {
            // If any of the channels is closed it means that for some reason the node failed so I ignore it and try to broadcast
            // in the remaining next nodes
            if write_to_node(node_sender, message.clone()).is_err() {
                amount_of_failed_nodes += 1;
                continue;
            }
        }
        // If all the nodes failed, it means that there are no nodes connected to the node --> Broadcasting failed
        if amount_of_failed_nodes == self.nodes_sender.len() {
            return Err(NodeCustomErrors::ThreadChannelError(
                "All nodes failed".to_string(),
            ));
        }
        Ok(())
    }

    /// Updates the value of the finish pointer that cuts the cycles of the nodes that are being listened to.
    /// It does the join in each one of the threads for each node that was being listened to.
    /// For each end of the channel to write to the nodes it performs drop() to close the channel.
    /// Returns Ok(()) if everything went well or specific Error otherwise.
    pub fn finish(&self) -> NodeMessageHandlerResult {
        *self
            .finish
            .write()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))? = true;
        let handles: Vec<JoinHandle<()>> = {
            let mut locked_handles = self
                .nodes_handle
                .lock()
                .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?;
            mem::take(&mut *locked_handles)
        };
        for handle in handles {
            handle
                .join()
                .map_err(|err| NodeCustomErrors::ThreadJoinError(format!("{:?}", err)))?;
        }
        for node_sender in self.nodes_sender.clone() {
            drop(node_sender);
        }
        Ok(())
    }

    /// Adds a new node to the list of nodes being listened to.
    /// The channel through which it will communicate with the node is passed as a parameter
    /// and the socket of the node you want to add. 
    /// Returns Ok(()) if everything went well or specific Error otherwise.
    pub fn add_connection(
        &mut self,
        log_sender: &LogSender,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        node_pointers: NodeDataPointers,
        connection: TcpStream,
    ) -> NodeMessageHandlerResult {
        let (tx, rx) = channel();
        self.nodes_sender.push(tx.clone());
        println!(
            "Node -{:?}- Listening for new blocks and transactions...\n NEW CONNECTION ADDED!!!",
            connection.peer_addr()
        );
        self.nodes_handle
            .lock()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .push(handle_messages_from_node(
                log_sender,
                ui_sender,
                (tx, rx),
                self.transactions_recieved.clone(),
                node_pointers,
                connection,
                Some(self.finish.clone()),
            ));
        Ok(())
    }
}

/// Creates a thread for a specific node and is responsible for performing the loop that listens
/// for new messages from the node. If necessary, it also writes to the node messages that arrive through the channel.
/// The finish pointer defines when the program ends and therefore the cycle of this function. Returns the JoinHandle of the thread
/// with what the loop returns. Ok(()) in case everything goes well or NodeHandlerError in case of any error.
pub fn handle_messages_from_node(
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    (tx, rx): (NodeSender, NodeReceiver),
    transactions_recieved: Arc<RwLock<Vec<[u8; 32]>>>,
    node_pointers: NodeDataPointers,
    mut node: TcpStream,
    finish: Option<Arc<RwLock<bool>>>,
) -> JoinHandle<()> {
    let log_sender = log_sender.clone();
    let ui_sender = ui_sender.clone();
    thread::spawn(move || {
        // If any error occurs it is saved in this variable
        let mut error: Option<NodeCustomErrors> = None;
        while !is_terminated(finish.clone()) {
            // If something was sent to write, it is written
            if let Ok(message) = rx.try_recv() {
                if let Err(err) = write_message_in_node(&mut node, &message) {
                    error = Some(err);
                    break;
                }
            }
            let header = match read_header(&mut node, finish.clone()) {
                Err(NodeCustomErrors::OtherError(_)) => {
                    // Not enough data available, continue
                    continue;
                }
                Err(err) => {
                    error = Some(err);
                    break;
                }
                Ok(header) => header,
            };

            let payload =
                match read_payload(&mut node, header.payload_size as usize, finish.clone()) {
                    Ok(payload) => payload,
                    Err(err) => {
                        error = Some(err);
                        break;
                    }
                };

            let command_name = get_header_command_name_as_str(header.command_name.as_str());

            match command_name {
                "headers" => handle_message(&mut error, || {
                    handle_headers_message(
                        &log_sender,
                        tx.clone(),
                        &payload,
                        node_pointers.blockchain.headers.clone(),
                        node_pointers.clone(),
                    )
                }),
                "getdata" => handle_message(&mut error, || {
                    handle_getdata_message(
                        &log_sender,
                        tx.clone(),
                        &payload,
                        node_pointers.blockchain.blocks.clone(),
                        node_pointers.accounts.clone(),
                    )
                }),
                "block" => handle_message(&mut error, || {
                    handle_block_message(&log_sender, &ui_sender, &payload, node_pointers.clone())
                }),
                "inv" => handle_message(&mut error, || {
                    handle_inv_message(tx.clone(), &payload, transactions_recieved.clone())
                }),
                "ping" => handle_message(&mut error, || handle_ping_message(tx.clone(), &payload)),
                "tx" => handle_message(&mut error, || {
                    handle_tx_message(
                        &log_sender,
                        &ui_sender,
                        &payload,
                        node_pointers.accounts.clone(),
                    )
                }),
                "getheaders" => handle_message(&mut error, || {
                    handle_getheaders_message(
                        tx.clone(),
                        &payload,
                        node_pointers.blockchain.headers.clone(),
                        node_pointers.clone(),
                    )
                }),
                _ => {
                    write_in_log(
                        &log_sender.message_log_sender,
                        format!(
                            "IGNORED -- Message: {} -- Node: {:?}",
                            header.command_name,
                            node.peer_addr()
                        )
                        .as_str(),
                    );
                    continue;
                }
            };
            if command_name != "inv" {
                // All messages are printed in the log_message except the inv (too many)
                write_in_log(
                    &log_sender.message_log_sender,
                    format!(
                        "Message received correctly: {} -- Node: {:?}",
                        command_name,
                        node.peer_addr()
                    )
                    .as_str(),
                );
            }
            // If any error occurs in the handling, it exits the cycle 
            if error.is_some() {
                break;
            }
        }
        // If an error occurs, it is documented in the error log sender
        if let Some(err) = error {
            write_in_log(
                &log_sender.error_log_sender,
                format!(
                    "NODE {:?} DISCONNECTED!! ERROR: {}",
                    node.peer_addr(),
                    err
                )
                .as_str(),
            );
        }
    })
}
/// Receives a mutable reference to the Option that indicates if an error occurred in the thread where messages are being listened to
/// and a function that handles a specific error. Calls the function and if it returns an error, sets the mutable reference
/// to the error that is returned.
fn handle_message<T, E>(error: &mut Option<E>, func: impl FnOnce() -> Result<T, E>) -> Option<T> {
    match func() {
        Ok(result) => Some(result),
        Err(err) => {
            *error = Some(err);
            None
        }
    }
}

/// Receives a &str that represents the name of a command of a header with its respective name
/// and the \0 until completing the 12 bytes. Returns a &str with the name of the message and removes the
/// extra \0
fn get_header_command_name_as_str(command: &str) -> &str {
    if let Some(first_null_char) = command.find('\0') {
        &command[0..first_null_char]
    } else {
        command
    }
}

/// Receives something that implements the Write trait and a vector of bytes that represents a message. It writes it and returns
/// Ok(()) in case of successful writing or a specific writing error otherwise.
pub fn write_message_in_node(node: &mut dyn Write, message: &[u8]) -> NodeMessageHandlerResult {
    node.write_all(message)
        .map_err(|err| NodeCustomErrors::WriteNodeError(err.to_string()))?;
    node.flush()
        .map_err(|err| NodeCustomErrors::WriteNodeError(err.to_string()))?;

    Ok(())
}

/// Reads a header message from the node socket and returns it or an error if it failed.
fn read_header(
    node: &mut dyn Read,
    finish: Option<Arc<RwLock<bool>>>,
) -> Result<HeaderMessage, NodeCustomErrors> {
    let mut buffer_num = [0; 24];
    if !is_terminated(finish.clone()) {
        match node.read_exact(&mut buffer_num) {
            Ok(_) => {} // Ok, continue
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                // Not enough data available
                return Err(NodeCustomErrors::OtherError(err.to_string()));
            }
            Err(err) => return Err(NodeCustomErrors::ReadNodeError(err.to_string())), // Unexpected error
        }
    }
    if is_terminated(finish) {
        // Returns any header so that it does not fail in the function in which read_header is called
        // and in this way break the while cycle well.
        return Ok(HeaderMessage::new("none".to_string(), None));
    }
    HeaderMessage::from_le_bytes(buffer_num)
        .map_err(|err| NodeCustomErrors::UnmarshallingError(err.to_string()))
}

/// Reads from the node socket until receiving the expected payload.
/// Returns the payload byte string or an error if it failed.
fn read_payload(
    node: &mut dyn Read,
    size: usize,
    finish: Option<Arc<RwLock<bool>>>,
) -> Result<Vec<u8>, NodeCustomErrors> {
    let mut payload_buffer_num: Vec<u8> = vec![0; size];
    while !is_terminated(finish.clone()) {
        match node.read_exact(&mut payload_buffer_num) {
            Ok(_) => break, // Ok, continue
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => continue, // Not enough data available, continue
            Err(err) => return Err(NodeCustomErrors::ReadNodeError(err.to_string())), // Unexpected error, return
        }
    }
    Ok(payload_buffer_num)
}

/// Receives an Arc pointing to a RwLock of a vector of TcpStreams and returns the last TcpStream node in the vector if there is
/// is, if not returns an error of the type CanNotRead.
fn get_last_node(nodes: Arc<RwLock<Vec<TcpStream>>>) -> Result<TcpStream, NodeCustomErrors> {
    let node = nodes
        .try_write()
        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
        .pop()
        .ok_or("Error no hay mas nodos para descargar los headers!\n")
        .map_err(|err| NodeCustomErrors::CanNotRead(err.to_string()))?;
    Ok(node)
}

/// Receives an Arc pointing to a vector of TcpStream and returns the length of the vector.
fn get_amount_of_nodes(nodes: Arc<RwLock<Vec<TcpStream>>>) -> Result<usize, NodeCustomErrors> {
    let amount_of_nodes = nodes
        .read()
        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
        .len();
    Ok(amount_of_nodes)
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn get_header_command_name_as_str_returns_correct_headers_command_name() {
        let header_command_name = "headers\0\0\0\0\0";
        assert_eq!(
            get_header_command_name_as_str(header_command_name),
            "headers"
        );
    }
    #[test]
    fn get_header_command_name_as_str_returns_correct_tx_command_name() {
        let header_command_name = "tx\0\0\0\0\0\0\0\0\0\0";
        assert_eq!(get_header_command_name_as_str(header_command_name), "tx");
    }
}
