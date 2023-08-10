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


/// # Blocks download
/// Downloads the blocks concurrently.
/// ### Receives:
/// - The reference to the list of nodes connected to.
/// - The reference to the hashmap of blocks where they will be stored
/// - The reference to the block headers downloaded
/// - The channel where it receives the block headers
/// - The channel where it returns the block headers when it can't download them
/// ### Error handling:
/// It tries to download the blocks from another node in the following cases:
/// - It couldn't send the request of the blocks
/// - It couldn't receive the block
/// ### Returns:
/// - Ok or an error if it can't complete the download
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
                "The list has 0 elements!".to_string(),
            ));
        }
        // should have received 2000 headers
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
            write_in_log(&log_sender.info_log_sender, format!("All the blocks were downloaded correctly! DOWNLOADED BLOCKS: {}\n", amount_of_blocks).as_str());
            return Ok(());
        }
    }
    Ok(())
}

/// Creates the thread from which a vec of blocks will be downloaded.
/// Returns the handle of the created thread or an error if it can't be created.
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
    // The thread should receive 250 headers
    write_in_log(
        &log_sender.info_log_sender,
        format!("{:?} Blocks will be downloaded from the node {:?}", block_headers.len(), node.peer_addr()).as_str(),
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
    //  Chunks of 16 blocks
    let mut inventory = vec![];
    for block in blocks_chunk_to_download {
        inventory.push(Inventory::new_block(block.hash()));
    }
    match GetDataMessage::new(inventory).write_to(node) {
        Ok(_) => Ok(()),
        Err(err) => {
            write_in_log(&log_sender.error_log_sender,format!("Error: {:?} amount of blocks can't be requested from the node: {:?}. I'll ask another node", blocks_chunk_to_download.len(), node.peer_addr()).as_str());
            try_to_download_blocks_from_other_node(tx, blocks_to_download)?;
            // Fails to send the message, I have to try with another node
            // If I return, I finish the thread.
            // I have to send all the blocks that the thread had
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
    // Receive the 16 (or less) blocks
    let mut current_blocks: Vec<Block> = Vec::new();
    for _ in 0..blocks_chunk_to_download.len() {
        let block = match BlockMessage::read_from(log_sender, node) {
            Ok(block) => block,
            Err(err) => {
                write_in_log(&log_sender.error_log_sender,format!("Error: {:?} amount of blocks can't be received from the node: {:?}. I'll ask another node", blocks_chunk_to_download.len(), node.peer_addr()).as_str());
                try_to_download_blocks_from_other_node(tx, blocks_to_download)?;
                // Fails to receive the message, I have to try with another node
                return Err(NodeCustomErrors::ReadNodeError(format!(
                    "Error at receiving `block` message: {:?}",
                    err
                )));
            }
        };
        let validation_result = block.validate();
        if !validation_result.0 {
            write_in_log(&log_sender.error_log_sender,format!("The block didn't pass the validation. {:?}. I'll ask another node and discard this one.", validation_result.1).as_str());
            try_to_download_blocks_from_other_node(tx, blocks_to_download)?;
            return Err(NodeCustomErrors::ReadNodeError(format!(
                "Error at receiving `block` message: {:?}",
                validation_result.1
            )));
        }
        current_blocks.push(block);
    }
    Ok(current_blocks)
}

/// Download all the blocks from a single node
/// Returns error in case of failure
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
            "{:?} blocks will be downloaded from the node {:?}",
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

/// Receives a vec of block headers and returns a vec of vecs of block headers, where each vec has the same amount of elements.
/// Separates them into chunks of equal size.
fn divide_blocks_to_download_in_equal_chunks(
    blocks_to_download: Vec<BlockHeader>,
    n_threads: usize,
) -> Arc<RwLock<Vec<Vec<BlockHeader>>>> {
    let chunk_size = (blocks_to_download.len() as f64 / n_threads as f64).ceil() as usize;
    // divides the vec into 8 with the same length (or same length but the last with less)
    let blocks_to_download_chunks = Arc::new(RwLock::new(
        blocks_to_download
            .chunks(chunk_size)
            .map(|chunk| chunk.to_vec())
            .collect::<Vec<_>>(),
    ));
    blocks_to_download_chunks
}

/// Receives a hashmap of blocks and returns the amount of blocks in it
/// Error in case of not being able to read it
pub fn amount_of_blocks(
    blocks: &Arc<RwLock<HashMap<[u8; 32], Block>>>,
) -> Result<usize, NodeCustomErrors> {
    let amount_of_blocks = blocks
        .read()
        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
        .len();
    Ok(amount_of_blocks)
}

/// Receives a pointer to a hashmap of blocks and a hashmap of downloaded blocks and adds them to the local hashmap of blocks
/// in case of not being able to access the local hashmap of blocks returns error
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
        format!("DOWNLOADING BLOCKS: {:?} blocks downloaded", amount_of_blocks(&blocks)?).as_str(),
    );
    let amount_of_blocks = amount_of_blocks(&blocks)?;
    println!("{:?} blocks downloaded", amount_of_blocks);
    let total_blocks_to_download =
        amount_of_headers(&headers)? - config.height_first_block_to_download;
    send_event_to_ui(
        ui_sender,
        UIEvent::UpdateBlocksDownloaded(amount_of_blocks, total_blocks_to_download),
    );
    Ok(())
}

/// Sends through the channel the headers received by parameter so that the respective blocks are downloaded from another node
/// Returns error if the channel is closed
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
