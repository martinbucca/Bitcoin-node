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
/// Struct para controlar todos los nodos conectados al nuestro. Escucha permanentemente
/// a estos y decide que hacer con los mensajes que llegan y con los que tiene que escribir
pub struct NodeMessageHandler {
    nodes_handle: Arc<Mutex<Vec<JoinHandle<()>>>>,
    nodes_sender: Vec<NodeSender>,
    transactions_recieved: Arc<RwLock<Vec<[u8; 32]>>>,
    finish: Arc<RwLock<bool>>,
}

impl NodeMessageHandler {
    /// Recibe la informacion que tiene el nodo (headers, bloques y nodos conectados)
    /// y se encarga de crear un thread por cada nodo y lo deja esuchando mensajes
    /// y handleandolos de forma oportuna. Si ocurre algun error devuelve un Error del enum
    /// NodeCustomErrors y en caso contrario devuelve el nuevo struct
    /// NodeMessageHandler con sus respectivos campos
    pub fn new(
        log_sender: &LogSender,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        node_pointers: NodeDataPointers,
    ) -> Result<Self, NodeCustomErrors> {
        write_in_log(
            &log_sender.info_log_sender,
            "Empiezo a escuchar por nuevos bloques y transaccciones",
        );
        let finish = Arc::new(RwLock::new(false));
        let mut nodes_handle: Vec<JoinHandle<()>> = vec![];
        let cant_nodos = get_amount_of_nodes(node_pointers.connected_nodes.clone())?;
        let mut nodes_sender = vec![];
        // Lista de transacciones recibidas para no recibir las mismas de varios nodos
        let transactions_recieved: Arc<RwLock<Vec<[u8; 32]>>> = Arc::new(RwLock::new(Vec::new()));
        for _ in 0..cant_nodos {
            let (tx, rx) = channel();
            nodes_sender.push(tx.clone());
            let node = get_last_node(node_pointers.connected_nodes.clone())?;
            println!(
                "Nodo -{:?}- Escuchando por nuevos bloques...\n",
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

    /// Recibe un vector de bytes que representa un mensaje serializado y se lo manda a cada canal que esta esperando para escribir en un nodo
    /// De esta manera se broadcastea el mensaje a todos los nodos conectados.
    /// Devuelve Ok(()) en caso exitoso o un error ThreadChannelError en caso contrario
    pub fn broadcast_to_nodes(&self, message: Vec<u8>) -> NodeMessageHandlerResult {
        let mut amount_of_failed_nodes = 0;
        for node_sender in &self.nodes_sender {
            // si alguno de los channels esta cerrado significa que por alguna razon el nodo fallo entonces lo ignoro y pruebo broadcastear
            // en los siguientes nodos restantes
            if write_to_node(node_sender, message.clone()).is_err() {
                amount_of_failed_nodes += 1;
                continue;
            }
        }
        // Si de todos los nodos, no se le pudo enviar a ninguno --> falla el broadcasting
        if amount_of_failed_nodes == self.nodes_sender.len() {
            return Err(NodeCustomErrors::ThreadChannelError(
                "Todos los channels cerrados, no se pudo boradcastear tx".to_string(),
            ));
        }
        Ok(())
    }

    /// Se encarga de actualizar el valor del puntero finish que corta los ciclos de los nodos que estan siendo esuchados.
    /// Hace el join en cada uno de los threads por cada nodo que estaba siendo escuchado.
    /// A cada extremo del channel para escribir en los nodos realiza drop() para que se cierre el channel.
    /// Devuelve Ok(()) en caso de salir todo bien o Error especifico en caso contrario
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

    /// Se encarga de agregar un nuevo nodo a la lista de nodos que estan siendo escuchados.
    /// Se le pasa como parametro el canal por el cual se va a comunicar con el nodo
    /// y el socket del nodo que se quiere agregar
    /// Devuelve Ok(()) en caso de salir todo bien o Error especifico en caso contrario
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
            "Nodo -{:?}- Escuchando por nuevos bloques...\n NUEVA CONECCION AGREGADA!!!",
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

/// Funcion encargada de crear un thread para un nodo especifico y se encarga de realizar el loop que escucha
/// por nuevos mensajes del nodo. En caso de ser necesario tambien escribe al nodo mensajes que le llegan por el channel.
/// El puntero finish define cuando el programa termina y por lo tanto el ciclo de esta funcion. Devuelve el JoinHandle del thread
/// con lo que devuelve el loop. Ok(()) en caso de salir todo bien o NodeHandlerError en caso de algun error.
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
        // si ocurre algun error se guarda en esta variable
        let mut error: Option<NodeCustomErrors> = None;
        while !is_terminated(finish.clone()) {
            // Veo si mandaron algo para escribir
            if let Ok(message) = rx.try_recv() {
                if let Err(err) = write_message_in_node(&mut node, &message) {
                    error = Some(err);
                    break;
                }
            }
            let header = match read_header(&mut node, finish.clone()) {
                Err(NodeCustomErrors::OtherError(_)) => {
                    //No hay suficientes datos disponibles, continuar
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
                            "IGNORADO -- Recibo: {} -- Nodo: {:?}",
                            header.command_name,
                            node.peer_addr()
                        )
                        .as_str(),
                    );
                    continue;
                }
            };
            if command_name != "inv" {
                // Se imprimen en el log_message todos los mensajes menos el inv
                write_in_log(
                    &log_sender.message_log_sender,
                    format!(
                        "Recibo correctamente: {} -- Nodo: {:?}",
                        command_name,
                        node.peer_addr()
                    )
                    .as_str(),
                );
            }
            // si ocurrio un error en el handleo salgo del ciclo
            if error.is_some() {
                break;
            }
        }
        // si ocurrio un error lo documento en el log sender de errores
        if let Some(err) = error {
            write_in_log(
                &log_sender.error_log_sender,
                format!(
                    "NODO {:?} DESCONECTADO!! OCURRIO UN ERROR: {}",
                    node.peer_addr(),
                    err
                )
                .as_str(),
            );
        }
    })
}
/// Recibe una referencia mutable al Option que indica si ocurrio un error en el thread en donde se estan escuchando
/// mensajes y una funcion que handlea un error especifico. Llama a la funcion y si devuelve un error setea la referencia mutable
/// al error que se devuelve
fn handle_message<T, E>(error: &mut Option<E>, func: impl FnOnce() -> Result<T, E>) -> Option<T> {
    match func() {
        Ok(result) => Some(result),
        Err(err) => {
            *error = Some(err);
            None
        }
    }
}

/// Recibe un &str que representa el nombre de un comando de un header con su respectivo nombre
/// y los \0 hasta completar los 12 bytes. Devuelve un &str con el nombre del mensaje y le quita los
/// \0 extras
fn get_header_command_name_as_str(command: &str) -> &str {
    if let Some(first_null_char) = command.find('\0') {
        &command[0..first_null_char]
    } else {
        command
    }
}

/// Recibe algo que implemente el trait Write y un vector de bytes que representa un mensaje. Lo escribe y devuevle
/// Ok(()) en caso de que se escriba exitosamente o un error especifico de escritura en caso contrarios
pub fn write_message_in_node(node: &mut dyn Write, message: &[u8]) -> NodeMessageHandlerResult {
    node.write_all(message)
        .map_err(|err| NodeCustomErrors::WriteNodeError(err.to_string()))?;
    node.flush()
        .map_err(|err| NodeCustomErrors::WriteNodeError(err.to_string()))?;

    Ok(())
}

/// Se mantiene leyendo del socket del nodo hasta recibir el header message.
/// Devuelve el HeaderMessage o un error si falló.
fn read_header(
    node: &mut dyn Read,
    finish: Option<Arc<RwLock<bool>>>,
) -> Result<HeaderMessage, NodeCustomErrors> {
    let mut buffer_num = [0; 24];
    if !is_terminated(finish.clone()) {
        match node.read_exact(&mut buffer_num) {
            Ok(_) => {} // Lectura exitosa, continuar
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                //No hay suficientes datos disponibles, continuar esperando
                return Err(NodeCustomErrors::OtherError(err.to_string()));
            }
            Err(err) => return Err(NodeCustomErrors::ReadNodeError(err.to_string())), // Error inesperado, devolverlo
        }
    }
    if is_terminated(finish) {
        // devuelvo un header cualquiera para que no falle en la funcion en la que se llama a read_header
        // y de esta manera cortar bien el ciclo while
        return Ok(HeaderMessage::new("none".to_string(), None));
    }
    HeaderMessage::from_le_bytes(buffer_num)
        .map_err(|err| NodeCustomErrors::UnmarshallingError(err.to_string()))
}

/// Se mantiene leyendo del socket del nodo hasta recibir el payload esperado.
/// Devuelve el la cadena de bytes del payload o un error si falló.
fn read_payload(
    node: &mut dyn Read,
    size: usize,
    finish: Option<Arc<RwLock<bool>>>,
) -> Result<Vec<u8>, NodeCustomErrors> {
    let mut payload_buffer_num: Vec<u8> = vec![0; size];
    while !is_terminated(finish.clone()) {
        match node.read_exact(&mut payload_buffer_num) {
            Ok(_) => break, // Lectura exitosa, salimos del bucle
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => continue, // No hay suficientes datos disponibles, continuar esperando
            Err(err) => return Err(NodeCustomErrors::ReadNodeError(err.to_string())), // Error inesperado, devolverlo
        }
    }
    Ok(payload_buffer_num)
}

/// Recibe un Arc apuntando a un RwLock de un vector de TcpStreams y devuelve el ultimo nodo TcpStream del vector si es que
/// hay, si no devuelve un error del tipo BroadcastingError
fn get_last_node(nodes: Arc<RwLock<Vec<TcpStream>>>) -> Result<TcpStream, NodeCustomErrors> {
    let node = nodes
        .try_write()
        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
        .pop()
        .ok_or("Error no hay mas nodos para descargar los headers!\n")
        .map_err(|err| NodeCustomErrors::CanNotRead(err.to_string()))?;
    Ok(node)
}

/// Recibe un Arc apuntando a un vector de TcpStream y devuelve el largo del vector
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
