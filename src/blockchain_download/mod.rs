use gtk::glib;

use self::blocks_download::{download_blocks, download_blocks_single_node};
use self::headers_download::{download_missing_headers, get_initial_headers};
use self::utils::{get_amount_of_headers_and_blocks, get_node, join_threads, return_node_to_vec};
use super::blocks::block::Block;
use super::blocks::block_header::BlockHeader;
use super::config::Config;
use super::logwriter::log_writer::{write_in_log, LogSender};
use crate::blockchain::Blockchain;
use crate::custom_errors::NodeCustomErrors;
use crate::gtk::ui_events::{send_event_to_ui, UIEvent};
use crate::utxo_tuple::UtxoTuple;
use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, RwLock};
use std::{thread, vec};
mod blocks_download;
pub(crate) mod headers_download;
mod utils;

type UtxoSetPointer = Arc<RwLock<HashMap<[u8; 32], UtxoTuple>>>;
type BlocksAndHeaders = (
    Arc<RwLock<HashMap<[u8; 32], Block>>>,
    Arc<RwLock<Vec<BlockHeader>>>,
);
// Gensis block header hardcoded to start the download (this is the first block of the blockchain)
// data taken from: https://en.bitcoin.it/wiki/Genesis_block
const GENESIS_BLOCK_HEADER: BlockHeader = BlockHeader {
    version: 1,
    previous_block_header_hash: [0; 32],
    merkle_root_hash: [
        59, 163, 237, 253, 122, 123, 18, 178, 122, 199, 44, 62, 103, 118, 143, 97, 127, 200, 27,
        195, 136, 138, 81, 50, 58, 159, 184, 170, 75, 30, 94, 74,
    ],
    time: 1296677802,
    n_bits: 486604799,
    nonce: 414098458,
};

/// Recieves a list of TcpStreams that are the connection with nodes already established and downloads
/// all the headers from the blockchain and the blocks from a config date. Returns the headers and blocks in
/// two separete lists in case of exit or an error in case of faliure
pub fn initial_block_download(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    nodes: Arc<RwLock<Vec<TcpStream>>>,
) -> Result<Blockchain, NodeCustomErrors> {
    write_in_log(
        &log_sender.info_log_sender,
        "EMPIEZA DESCARGA INICIAL DE BLOQUES",
    );
    // el vector de headers empieza con el header del bloque genesis
    let headers = vec![GENESIS_BLOCK_HEADER];
    let pointer_to_headers = Arc::new(RwLock::new(headers));
    let blocks: HashMap<[u8; 32], Block> = HashMap::new();
    let pointer_to_blocks = Arc::new(RwLock::new(blocks));
    let utxo_set: UtxoSetPointer = Arc::new(RwLock::new(HashMap::new()));
    let mut heights_hashmap: HashMap<[u8; 32], usize> = HashMap::new();
    heights_hashmap.insert([0u8; 32], 0); // genesis hash
    let header_heights: Arc<RwLock<HashMap<[u8; 32], usize>>> =
        Arc::new(RwLock::new(heights_hashmap));

    get_initial_headers(
        config,
        log_sender,
        ui_sender,
        pointer_to_headers.clone(),
        header_heights.clone(),
        nodes.clone(),
    )?;
    let amount_of_nodes = nodes
        .read()
        .map_err(|err| NodeCustomErrors::LockError(format!("{:?}", err)))?
        .len();

    if config.ibd_single_node || amount_of_nodes < 2 {
        download_full_blockchain_from_single_node(
            config,
            log_sender,
            ui_sender,
            nodes,
            (pointer_to_blocks.clone(), pointer_to_headers.clone()),
            header_heights.clone(),
            utxo_set.clone(),
        )?;
    } else {
        download_full_blockchain_from_multiple_nodes(
            config,
            log_sender,
            ui_sender,
            nodes,
            (pointer_to_blocks.clone(), pointer_to_headers.clone()),
            header_heights.clone(),
            utxo_set.clone(),
        )?;
    }

    let (amount_of_headers, amount_of_blocks) =
        get_amount_of_headers_and_blocks(&pointer_to_headers, &pointer_to_blocks)?;
    write_in_log(
        &log_sender.info_log_sender,
        format!("TOTAL DE HEADERS DESCARGADOS: {}", amount_of_headers).as_str(),
    );
    write_in_log(
        &log_sender.info_log_sender,
        format!("TOTAL DE BLOQUES DESCARGADOS: {}\n", amount_of_blocks).as_str(),
    );
    Ok(Blockchain::new(
        pointer_to_headers,
        pointer_to_blocks,
        header_heights,
        utxo_set,
    ))
}

/// Se encarga de descargar todos los headers y bloques de la blockchain en multiples thread, en un thread descarga los headers
/// y en el otro a medida que se van descargando los headers va pidiendo los bloques correspondientes.
/// Devuelve error en caso de falla.
fn download_full_blockchain_from_multiple_nodes(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    nodes: Arc<RwLock<Vec<TcpStream>>>,
    (blocks, headers): BlocksAndHeaders,
    header_heights: Arc<RwLock<HashMap<[u8; 32], usize>>>,
    utxo_set: UtxoSetPointer,
) -> Result<(), NodeCustomErrors> {
    // channel to comunicate headers download thread with blocks download thread
    let (tx, rx) = channel();
    let mut threads_handle = vec![];
    let config_cloned = config.clone();
    let log_sender_cloned = log_sender.clone();
    let nodes_cloned = nodes.clone();
    let headers_cloned = headers.clone();
    let tx_cloned = tx.clone();
    let ui_sender_clone = ui_sender.clone();
    threads_handle.push(thread::spawn(move || {
        download_missing_headers(
            &config_cloned,
            &log_sender_cloned,
            &ui_sender_clone,
            nodes_cloned,
            headers_cloned,
            header_heights,
            tx_cloned,
        )
    }));
    let config = config.clone();
    let log_sender = log_sender.clone();
    let ui_sender = ui_sender.clone();
    let (tx_utxo_set, rx_utxo_set) = channel();
    let utxo_set_clone = utxo_set;
    let join_handle = thread::spawn(move || -> Result<(), NodeCustomErrors> {
        load_utxo_set(rx_utxo_set, utxo_set_clone)
    });
    threads_handle.push(thread::spawn(move || {
        download_blocks(
            &config,
            &log_sender,
            &ui_sender,
            nodes,
            (blocks, headers),
            (tx, rx),
            tx_utxo_set,
        )
    }));
    join_threads(threads_handle)?;
    join_handle
        .join()
        .map_err(|err| NodeCustomErrors::ThreadJoinError(format!("{:?}", err)))??;
    Ok(())
}

/// Se encarga de descargar todos los headers y bloques de la blockchain en un solo thread, primero descarga todos los headers
/// y luego descarga todos los bloques. Devuelve error en caso de falla.
fn download_full_blockchain_from_single_node(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    nodes: Arc<RwLock<Vec<TcpStream>>>,
    (blocks, headers): BlocksAndHeaders,
    header_heights: Arc<RwLock<HashMap<[u8; 32], usize>>>,
    utxo_set: UtxoSetPointer,
) -> Result<(), NodeCustomErrors> {
    let (tx, rx) = channel();
    download_missing_headers(
        config,
        log_sender,
        ui_sender,
        nodes.clone(),
        headers.clone(),
        header_heights,
        tx,
    )?;
    let mut node = get_node(nodes.clone())?;
    let (tx_utxo_set, rx_utxo_set) = channel();
    let utxo_set_clone = utxo_set;
    let join_handle = thread::spawn(move || -> Result<(), NodeCustomErrors> {
        load_utxo_set(rx_utxo_set, utxo_set_clone)
    });
    send_event_to_ui(ui_sender, UIEvent::StartDownloadingBlocks);
    for blocks_to_download in rx {
        download_blocks_single_node(
            config,
            log_sender,
            ui_sender,
            (blocks.clone(), headers.clone()),
            blocks_to_download,
            &mut node,
            tx_utxo_set.clone(),
        )?;
    }
    return_node_to_vec(nodes, node)?;
    drop(tx_utxo_set);
    join_handle
        .join()
        .map_err(|err| NodeCustomErrors::ThreadJoinError(format!("{:?}", err)))??;
    Ok(())
}

/// Actualiza el utxo_set a medida que recibe los bloques por el channel
fn load_utxo_set(
    rx: Receiver<Vec<Block>>,
    utxo_set: UtxoSetPointer,
) -> Result<(), NodeCustomErrors> {
    for blocks in rx {
        for block in blocks {
            block
                .give_me_utxos(utxo_set.clone())
                .map_err(|err| NodeCustomErrors::UtxoError(err.to_string()))?;
        }
    }
    Ok(())
}


