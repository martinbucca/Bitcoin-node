use gtk::glib;

use crate::{
    blockchain_download::headers_download::amount_of_headers,
    blocks::{block::Block, block_header::BlockHeader},
    config::Config,
    custom_errors::NodeCustomErrors,
    gtk::ui_events::{send_event_to_ui, UIEvent},
    logwriter::log_writer::{write_in_log, LogSender},
    messages::{
        block_message::BlockMessage, get_data_message::GetDataMessage, inventory::Inventory,
    },
};
use std::{
    collections::HashMap,
    net::TcpStream,
    sync::{
        mpsc::{Receiver, Sender},
        Arc, RwLock,
    },
    thread::{self, JoinHandle},
};

use super::{
    get_amount_of_headers_and_blocks, join_threads,
    utils::{get_node, return_node_to_vec},
};

type BlocksAndHeaders = (
    Arc<RwLock<HashMap<[u8; 32], Block>>>,
    Arc<RwLock<Vec<BlockHeader>>>,
);

type BlocksTuple = (
    Vec<BlockHeader>,
    Arc<RwLock<HashMap<[u8; 32], Block>>>,
    Arc<RwLock<Vec<BlockHeader>>>,
);

/// # Descarga de bloques
/// Realiza la descarga de bloques de forma concurrente.
/// ### Recibe:
/// - La referencia a la lista de nodos a los que se conectar.
/// - La referencia a la lista de bloques donde los almacenará
/// - La referencia a los block headers descargados
/// - El channel por donde recibe los block headers
/// - El channel por donde devuelve los block headers cuando no los puede descargar
///
/// ### Manejo de errores:
/// Vuelve a intentar la descarga con un nuevo nodo, en los siguientes casos:
/// - No se pudo realizar la solicitud de los bloques
/// - No se pudo recibir el bloque
///
/// ### Devuelve:
/// - Ok o un error si no se puede completar la descarga
pub fn download_blocks(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    nodes: Arc<RwLock<Vec<TcpStream>>>,
    (blocks, headers): BlocksAndHeaders,
    (tx, rx): (Sender<Vec<BlockHeader>>, Receiver<Vec<BlockHeader>>),
    tx_utxo_set: Sender<Vec<Block>>,
) -> Result<(), NodeCustomErrors> {
    // recieves in the channel the vec of headers sent by the function downloading headers
    for blocks_to_download in rx {
        if blocks_to_download.is_empty() {
            return Err(NodeCustomErrors::ThreadChannelError(
                "Se recibio una lista con 0 elementos!".to_string(),
            ));
        }
        // acá recibo 2000 block headers
        let mut n_threads = config.n_threads;
        if blocks_to_download.len() <= config.blocks_download_per_node {
            n_threads = 1;
        }
        let blocks_to_download_chunks =
            divide_blocks_to_download_in_equal_chunks(blocks_to_download, n_threads);
        let mut join_handles = vec![];
        for blocks_to_download_chunk in blocks_to_download_chunks
            .read()
            .map_err(|err| NodeCustomErrors::CanNotRead(err.to_string()))?
            .iter()
        {
            join_handles.push(download_blocks_chunck(
                config,
                log_sender,
                ui_sender,
                (blocks_to_download_chunk.clone(), headers.clone()),
                nodes.clone(),
                (tx.clone(), tx_utxo_set.clone()),
                blocks.clone(),
            )?);
        }
        join_threads(join_handles)?;
        let (amount_of_headers, amount_of_blocks) =
            get_amount_of_headers_and_blocks(&headers, &blocks)?;
        let total_blocks_to_download = amount_of_headers - config.height_first_block_to_download;
        if amount_of_blocks == total_blocks_to_download {
            write_in_log(&log_sender.info_log_sender, format!("Se terminaron de descargar todos los bloques correctamente! BLOQUES DESCARGADOS: {}\n", amount_of_blocks).as_str());
            return Ok(());
        }
    }
    Ok(())
}

/// Se encarga de crear el thread desde el cual se van a descargar un vector de bloques.
/// Devuelve el handle del thread creado o error en caso de no poder crearlo.
fn download_blocks_chunck(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    (block_headers, headers): (Vec<BlockHeader>, Arc<RwLock<Vec<BlockHeader>>>),
    nodes: Arc<RwLock<Vec<TcpStream>>>,
    (tx, tx_utxo_set): (Sender<Vec<BlockHeader>>, Sender<Vec<Block>>),
    blocks: Arc<RwLock<HashMap<[u8; 32], Block>>>,
) -> Result<JoinHandle<Result<(), NodeCustomErrors>>, NodeCustomErrors> {
    let config_cloned = config.clone();
    let log_sender_cloned = log_sender.clone();
    let node = get_node(nodes.clone())?;
    let ui_sender = ui_sender.clone();
    Ok(thread::spawn(move || {
        download_blocks_single_thread(
            &config_cloned,
            &log_sender_cloned,
            &ui_sender,
            (block_headers, blocks, headers),
            node,
            (tx, tx_utxo_set),
            nodes,
        )
    }))
}

/// Downloads all the blocks from the same node, in the same thread.
/// The blocks are stored in the blocks list received by parameter.
/// In the end, the node is also return to the list of nodes
/// ## Errors
/// In case of Read or Write error on the node, the function is terminated, discarding the problematic node.
/// The downloaded blocks upon the error are discarded, so the whole block chunk can be downloaded again from another node
/// In other cases, it returns error.
fn download_blocks_single_thread(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    (block_headers, blocks, headers): BlocksTuple,
    mut node: TcpStream,
    (tx, tx_utxo_set): (Sender<Vec<BlockHeader>>, Sender<Vec<Block>>),
    nodes: Arc<RwLock<Vec<TcpStream>>>,
) -> Result<(), NodeCustomErrors> {
    let mut current_blocks: HashMap<[u8; 32], Block> = HashMap::new();
    // el thread recibe 250 bloques
    write_in_log(
        &log_sender.info_log_sender,
        format!(
            "Voy a descargar {:?} bloques del nodo {:?}",
            block_headers.len(),
            node.peer_addr()
        )
        .as_str(),
    );
    for blocks_to_download in block_headers.chunks(config.blocks_download_per_node) {
        match request_blocks_from_node(
            log_sender,
            &mut node,
            blocks_to_download,
            block_headers.clone(),
            Some(tx.clone()),
        ) {
            Ok(_) => {}
            Err(NodeCustomErrors::WriteNodeError(_)) => return Ok(()),
            Err(error) => return Err(error),
        }
        let received_blocks = match receive_requested_blocks_from_node(
            log_sender,
            &mut node,
            blocks_to_download,
            block_headers.clone(),
            Some(tx.clone()),
        ) {
            Ok(blocks) => blocks,
            Err(NodeCustomErrors::ReadNodeError(_)) => return Ok(()),
            Err(error) => return Err(error),
        };
        tx_utxo_set
            .send(received_blocks.clone())
            .map_err(|err| NodeCustomErrors::ThreadChannelError(err.to_string()))?;
        for block in received_blocks.into_iter() {
            current_blocks.insert(block.hash(), block);
        }
    }
    add_blocks_downloaded_to_local_blocks(
        config,
        log_sender,
        ui_sender,
        headers,
        blocks,
        current_blocks,
    )?;
    return_node_to_vec(nodes, node)?;
    Ok(())
}

/// Requests the blocks to the node.
/// ## Errors
/// In case of error while sending the message, it returns the block headers back to the channel so
/// they can be downloaded from another node. If this cannot be done, returns an error.
fn request_blocks_from_node(
    log_sender: &LogSender,
    node: &mut TcpStream,
    blocks_chunk_to_download: &[BlockHeader],
    blocks_to_download: Vec<BlockHeader>,
    tx: Option<Sender<Vec<BlockHeader>>>,
) -> Result<(), NodeCustomErrors> {
    //  Acá ya separé los 250 en chunks de 16 para las llamadas
    let mut inventory = vec![];
    for block in blocks_chunk_to_download {
        inventory.push(Inventory::new_block(block.hash()));
    }
    match GetDataMessage::new(inventory).write_to(node) {
        Ok(_) => Ok(()),
        Err(err) => {
            write_in_log(&log_sender.error_log_sender,format!("Error: No puedo pedir {:?} cantidad de bloques del nodo: {:?}. Se los voy a pedir a otro nodo", blocks_chunk_to_download.len(), node.peer_addr()).as_str());
            try_to_download_blocks_from_other_node(tx, blocks_to_download)?;
            // falló el envio del mensaje, tengo que intentar con otro nodo
            // si hago return, termino el thread.
            // tengo que enviar todos los bloques que tenía ese thread
            Err(NodeCustomErrors::WriteNodeError(format!("{:?}", err)))
        }
    }
}

/// Receives the blocks previously requested to the node.
/// Returns an array with the blocks.
/// In case of error while receiving the message, it returns the block headers back to the channel so
/// they can be downloaded from another node. If this cannot be done, returns an error.
fn receive_requested_blocks_from_node(
    log_sender: &LogSender,
    node: &mut TcpStream,
    blocks_chunk_to_download: &[BlockHeader],
    blocks_to_download: Vec<BlockHeader>,
    tx: Option<Sender<Vec<BlockHeader>>>,
) -> Result<Vec<Block>, NodeCustomErrors> {
    // Acá tengo que recibir los 16 bloques (o menos) de la llamada
    let mut current_blocks: Vec<Block> = Vec::new();
    for _ in 0..blocks_chunk_to_download.len() {
        let block = match BlockMessage::read_from(log_sender, node) {
            Ok(block) => block,
            Err(err) => {
                write_in_log(&log_sender.error_log_sender,format!("No puedo descargar {:?} de bloques del nodo: {:?}. Se los voy a pedir a otro nodo y descarto este. Error: {err}", blocks_chunk_to_download.len(), node.peer_addr()).as_str());
                try_to_download_blocks_from_other_node(tx, blocks_to_download)?;
                // falló la recepción del mensaje, tengo que intentar con otro nodo
                // termino el nodo con el return
                return Err(NodeCustomErrors::ReadNodeError(format!(
                    "Error al recibir el mensaje `block`: {:?}",
                    err
                )));
            }
        };
        let validation_result = block.validate();
        if !validation_result.0 {
            write_in_log(&log_sender.error_log_sender,format!("El bloque no pasó la validación. {:?}. Se los voy a pedir a otro nodo y descarto este.", validation_result.1).as_str());
            try_to_download_blocks_from_other_node(tx, blocks_to_download)?;
            return Err(NodeCustomErrors::ReadNodeError(format!(
                "Error al recibir el mensaje `block`: {:?}",
                validation_result.1
            )));
        }
        //block.set_utxos(); // seteo utxos de las transacciones del bloque
        current_blocks.push(block);
    }
    Ok(current_blocks)
}

/// Descarga todos los bloques desde un solo nodo
/// Devuelve error en caso de falla
pub fn download_blocks_single_node(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    (blocks, headers): BlocksAndHeaders,
    block_headers: Vec<BlockHeader>,
    node: &mut TcpStream,
    tx_utxo_set: Sender<Vec<Block>>,
) -> Result<(), NodeCustomErrors> {
    let mut current_blocks: HashMap<[u8; 32], Block> = HashMap::new();
    write_in_log(
        &log_sender.info_log_sender,
        format!(
            "Voy a descargar {:?} bloques del nodo {:?}",
            block_headers.len(),
            node.peer_addr()
        )
        .as_str(),
    );
    for blocks_to_download in block_headers.chunks(config.blocks_download_per_node) {
        request_blocks_from_node(
            log_sender,
            node,
            blocks_to_download,
            block_headers.clone(),
            None,
        )?;
        let received_blocks = receive_requested_blocks_from_node(
            log_sender,
            node,
            blocks_to_download,
            block_headers.clone(),
            None,
        )?;
        tx_utxo_set
            .send(received_blocks.clone())
            .map_err(|err| NodeCustomErrors::ThreadChannelError(err.to_string()))?;
        for block in received_blocks.into_iter() {
            current_blocks.insert(block.hash(), block);
        }
    }
    add_blocks_downloaded_to_local_blocks(
        config,
        log_sender,
        ui_sender,
        headers,
        blocks,
        current_blocks,
    )?;
    Ok(())
}

/*
***************************************************************************
************************ AUXILIAR FUNCTIONS *******************************
***************************************************************************
*/

/// Recibe un vector de block headers y devuelve un vector de vectores de block headers, donde cada vector tiene la misma cantidad de elementos.
/// Los separa en chunks de igual tamaño.
fn divide_blocks_to_download_in_equal_chunks(
    blocks_to_download: Vec<BlockHeader>,
    n_threads: usize,
) -> Arc<RwLock<Vec<Vec<BlockHeader>>>> {
    let chunk_size = (blocks_to_download.len() as f64 / n_threads as f64).ceil() as usize;
    // divides the vec into 8 with the same lenght (or same lenght but the last with less)
    let blocks_to_download_chunks = Arc::new(RwLock::new(
        blocks_to_download
            .chunks(chunk_size)
            .map(|chunk| chunk.to_vec())
            .collect::<Vec<_>>(),
    ));
    blocks_to_download_chunks
}

/// Recibe un hashmap de bloques y devuelve la cantidad de bloques que hay en el mismo
/// Error en caso de no poder leerlo
pub fn amount_of_blocks(
    blocks: &Arc<RwLock<HashMap<[u8; 32], Block>>>,
) -> Result<usize, NodeCustomErrors> {
    let amount_of_blocks = blocks
        .read()
        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
        .len();
    Ok(amount_of_blocks)
}

/// Recibe un puntero a un hashmap de bloques y un hashmap de bloques descargados y los agrega al hashmap de bloques local
/// en caso de no poder acceder al hashmap de bloques local devuelve error
pub fn add_blocks_downloaded_to_local_blocks(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    headers: Arc<RwLock<Vec<BlockHeader>>>,
    blocks: Arc<RwLock<HashMap<[u8; 32], Block>>>,
    downloaded_blocks: HashMap<[u8; 32], Block>,
) -> Result<(), NodeCustomErrors> {
    blocks
        .write()
        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
        .extend(downloaded_blocks);
    write_in_log(
        &log_sender.info_log_sender,
        format!("BLOQUES DESCARGADOS: {:?}", amount_of_blocks(&blocks)?).as_str(),
    );
    let amount_of_blocks = amount_of_blocks(&blocks)?;
    println!("{:?} bloques descargados", amount_of_blocks);
    let total_blocks_to_download =
        amount_of_headers(&headers)? - config.height_first_block_to_download;
    send_event_to_ui(
        ui_sender,
        UIEvent::ActualizeBlocksDownloaded(amount_of_blocks, total_blocks_to_download),
    );
    Ok(())
}

/// Envia por el channel los headers recibidos por parametro para que los respectivos bloques sean descargados desde otro nodo
/// Devuelve error en caso de que el channel este cerrado
fn try_to_download_blocks_from_other_node(
    tx: Option<Sender<Vec<BlockHeader>>>,
    headers_read: Vec<BlockHeader>,
) -> Result<(), NodeCustomErrors> {
    match tx {
        Some(tx) => {
            tx.send(headers_read)
                .map_err(|err| NodeCustomErrors::ThreadChannelError(err.to_string()))?;
        }
        None => return Ok(()),
    }
    Ok(())
}
