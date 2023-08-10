use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    net::TcpStream,
    path::Path,
    sync::{mpsc::Sender, Arc, RwLock},
};

use chrono::{TimeZone, Utc};
use gtk::glib;

use crate::{
    blocks::block_header::BlockHeader,
    config::Config,
    custom_errors::NodeCustomErrors,
    gtk::ui_events::{send_event_to_ui, UIEvent},
    logwriter::log_writer::{write_in_log, LogSender},
    messages::{getheaders_message::GetHeadersMessage, headers_message::HeadersMessage},
};

use super::{
    utils::{get_node, return_node_to_vec},
    GENESIS_BLOCK_HEADER,
};

const HEADERS_MESSAGE_SIZE: usize = 162003;

const GENESIS_BLOCK_HASH: [u8; 32] = [
    0x00, 0x00, 0x00, 0x00, 0x09, 0x33, 0xea, 0x01, 0xad, 0x0e, 0xe9, 0x84, 0x20, 0x97, 0x79, 0xba,
    0xae, 0xc3, 0xce, 0xd9, 0x0f, 0xa3, 0xf4, 0x08, 0x71, 0x95, 0x26, 0xf8, 0xd7, 0x7f, 0x49, 0x43,
];

/*
***************************************************************************
***************** INITIAL HEADERS AND PERSISTANCE *************************
***************************************************************************
*/

/// Downloads the first headers of the blockchain and saves them to disk.
/// If they are already saved, it reads them from there, otherwise
/// reads and saves them. If it is configured to read from disk, it will read from there.
/// If it is configured to read from the network, it will read from there and save to disk.
pub fn get_initial_headers(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    headers: Arc<RwLock<Vec<BlockHeader>>>,
    header_heights: Arc<RwLock<HashMap<[u8; 32], usize>>>,
    nodes: Arc<RwLock<Vec<TcpStream>>>,
) -> Result<(), NodeCustomErrors> {
    if config.read_headers_from_disk && Path::new(&config.headers_file).exists() {
        if let Err(err) = read_headers_from_disk(
            config,
            log_sender,
            ui_sender,
            headers.clone(),
            header_heights.clone(),
        ) {
            // si no se pudo descargar de disco, intento desde la red y guardo en disco
            // If it cannot be downloaded from disk, it tries from the network and saves to disk
            write_in_log(
                &log_sender.error_log_sender,
                format!("Error trying to read headers from disk: {}", err).as_str(),
            );
        } else {
            return Ok(());
        }
    }
    download_and_persist_headers(
        config,
        log_sender,
        ui_sender,
        headers,
        header_heights,
        nodes,
    )?;
    Ok(())
}

/// Lee los headers de disco y los guarda en el vector de headers.
/// Devuelve un error en caso de no poder leer el archivo correctamente.
/// Reads the headers from disk and saves them to the headers vector.
/// Returns an error if you cannot read the file correctly or Ok(()) otherwise.
fn read_headers_from_disk(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    headers: Arc<RwLock<Vec<BlockHeader>>>,
    header_heights: Arc<RwLock<HashMap<[u8; 32], usize>>>,
) -> Result<(), NodeCustomErrors> {
    write_in_log(
        &log_sender.info_log_sender,
        format!(
            "Start reading first {} headers from disk",
            config.headers_in_disk
        )
        .as_str(),
    );
    send_event_to_ui(ui_sender, UIEvent::StartDownloadingHeaders);
    let mut data: Vec<u8> = Vec::new();
    let mut file = File::open(&config.headers_file)
        .map_err(|err| NodeCustomErrors::OpeningFileError(err.to_string()))?;
    file.read_to_end(&mut data)
        .map_err(|err| NodeCustomErrors::ReadingFileError(err.to_string()))?;
    let mut amount = 0;
    let mut i = 0;
    while i < data.len() {
        amount += 2000;
        let mut message_bytes = Vec::new();
        message_bytes.extend_from_slice(&data[i..i + HEADERS_MESSAGE_SIZE]);
        let unmarshalled_headers = HeadersMessage::unmarshalling(&message_bytes)
            .map_err(|err| NodeCustomErrors::UnmarshallingError(err.to_string()))?;

        load_header_heights(&unmarshalled_headers, &header_heights, &headers)?;

        headers
            .write()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .extend_from_slice(&unmarshalled_headers);
        println!("{:?} headers read", amount);
        send_event_to_ui(
            ui_sender,
            UIEvent::UpdateHeadersDownloaded(amount as usize),
        );
        i += HEADERS_MESSAGE_SIZE;
    }
    write_in_log(
        &log_sender.info_log_sender,
        format!("{:?} headers read correctly from disk", amount).as_str(),
    );
    Ok(())
}

/// Loads the hashes of the headers into a hashmap to be able to obtain the height of a header in O(1).
pub fn load_header_heights(
    headers: &Vec<BlockHeader>,
    header_heights: &Arc<RwLock<HashMap<[u8; 32], usize>>>,
    headers_vec: &Arc<RwLock<Vec<BlockHeader>>>,
) -> Result<(), NodeCustomErrors> {
    let mut height = headers_vec
        .read()
        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
        .len();

    let mut header_heights_lock = header_heights
        .write()
        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?;

    for header in headers {
        header_heights_lock.insert(header.hash(), height);
        height += 1;
    }
    Ok(())
}

/// Descarga los primeros headers de la blockchain, crea el archivo para guardarlos y los guarda en disco
/// En caso de que un nodo falle en la descarga, intenta con otro siempre y cuando tenga peers disponibles
/// Devuelve un error en caso de no poder descargar los headers desde nignun nodo peer
/// Downloads the first headers of the blockchain, creates the file to save them and saves them to disk.
/// In case a node fails in the download, it tries with another as long as it has peers available. Returns
/// Ok(()) if it is downloaded correctly or an error otherwise.
fn download_and_persist_headers(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    headers: Arc<RwLock<Vec<BlockHeader>>>,
    header_heights: Arc<RwLock<HashMap<[u8; 32], usize>>>,
    nodes: Arc<RwLock<Vec<TcpStream>>>,
) -> Result<(), NodeCustomErrors> {
    write_in_log(
        &log_sender.info_log_sender,
        format!(
            "Start download of first {} headers to write them in disk",
            config.headers_in_disk
        )
        .as_str(),
    );
    send_event_to_ui(ui_sender, UIEvent::StartDownloadingHeaders);
    let mut file = File::create(&config.headers_file)
        .map_err(|err| NodeCustomErrors::OpeningFileError(err.to_string()))?;
    // get last node from list, if possible
    let mut node = get_node(nodes.clone())?;
    while let Err(err) = download_and_persist_initial_headers_from_node(
        config,
        log_sender,
        ui_sender,
        &mut node,
        headers.clone(),
        header_heights.clone(),
        &mut file,
    ) {
        write_in_log(
            &log_sender.error_log_sender,
            format!(
                "Download from node --{:?}-- fails, it is discarded. Error: {}",
                node.peer_addr(),
                err
            )
            .as_str(),
        );
        node = get_node(nodes.clone())?;
    }
    // return node that donwloaded the header again to the vec of nodes
    return_node_to_vec(nodes, node)?;
    Ok(())
}

/// Downloads the first headers (specified in the configuration file) from a node of the blockchain and saves them to disk.
/// The genesis block is not downloaded from the network, it already has it hardcoded. Returns an error if it cannot
/// download the headers successfully, otherwise Ok(()).
fn download_and_persist_initial_headers_from_node(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    node: &mut TcpStream,
    headers: Arc<RwLock<Vec<BlockHeader>>>,
    header_heights: Arc<RwLock<HashMap<[u8; 32], usize>>>,
    file: &mut File,
) -> Result<(), NodeCustomErrors> {
    write_in_log(
        &log_sender.info_log_sender,
        format!(
            "Start download of headers with node: {:?}\n",
            node.peer_addr()
        )
        .as_str(),
    );
    while headers
        .read()
        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
        .len()
        < config.headers_in_disk
    {
        request_headers_from_node(config, node, headers.clone())?;
        let headers_read = receive_and_persist_initial_headers_from_node(log_sender, node, file)?;
        load_header_heights(&headers_read, &header_heights, &headers)?;
        store_headers_in_local_headers_vec(log_sender, headers.clone(), &headers_read)?;
        let amount_of_headers = amount_of_headers(&headers)?;
        println!(
            "{:?} headers downloaded and saved in disk",
            amount_of_headers
        );
        send_event_to_ui(
            ui_sender,
            UIEvent::UpdateHeadersDownloaded(amount_of_headers),
        );
    }
    Ok(())
}

/// Receives the headers from the node and saves them to disk.
/// Returns an error if you cannot receive them correctly or Ok(()) otherwise.
fn receive_and_persist_initial_headers_from_node(
    log_sender: &LogSender,
    node: &mut TcpStream,
    file: &mut File,
) -> Result<Vec<BlockHeader>, NodeCustomErrors> {
    let headers: Vec<BlockHeader> = HeadersMessage::read_from_node_and_write_to_file(
        log_sender, node, None, file,
    )
    .map_err(|_| {
        NodeCustomErrors::BlockchainDownloadError(
            "Error trying to read and save headers in disk".to_string(),
        )
    })?;
    Ok(headers)
}

/*
***************************************************************************
******************* DOWNLOAD HEADERS FROM NETWORK *************************
***************************************************************************
*/

/// Downloads the headers of the blockchain from the connected nodes.
/// In case a node fails in the download, it tries with another as long as it has peers available. Returns
/// Ok(()) if it is downloaded correctly or an error otherwise.
pub fn download_missing_headers(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    nodes: Arc<RwLock<Vec<TcpStream>>>,
    headers: Arc<RwLock<Vec<BlockHeader>>>,
    header_heights: Arc<RwLock<HashMap<[u8; 32], usize>>>,
    tx: Sender<Vec<BlockHeader>>,
) -> Result<(), NodeCustomErrors> {
    // get last node from list, if possible
    let mut node = get_node(nodes.clone())?;
    while let Err(err) = download_missing_headers_from_node(
        config,
        log_sender,
        ui_sender,
        &mut node,
        headers.clone(),
        header_heights.clone(),
        tx.clone(),
    ) {
        write_in_log(
            &log_sender.error_log_sender,
            format!(
                "Download from node --{:?}-- fails, it is discarded. Error: {}",
                node.peer_addr(),
                err
            )
            .as_str(),
        );
        if let NodeCustomErrors::ThreadChannelError(_) = err {
            return Err(NodeCustomErrors::ThreadChannelError("Error the channel that comunicates the headers and blocks paralell download is closed".to_string()));
        }
        node = get_node(nodes.clone())?;
    }
    // return node again to the list of nodes
    return_node_to_vec(nodes, node)?;
    send_event_to_ui(
        ui_sender,
        UIEvent::FinsihDownloadingHeaders(amount_of_headers(&headers)?),
    );
    Ok(())
}

/// Downloads the headers from a particular node and saves them to the headers vector.
/// If the tx parameter is a Sender, it sends the headers it is downloading to the thread
/// that downloads blocks to be downloaded in parallel, otherwise it does not send anything.
/// Devuelve error en caso de falla.
fn download_missing_headers_from_node(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    node: &mut TcpStream,
    headers: Arc<RwLock<Vec<BlockHeader>>>,
    header_heights: Arc<RwLock<HashMap<[u8; 32], usize>>>,
    tx: Sender<Vec<BlockHeader>>,
) -> Result<(), NodeCustomErrors> {
    write_in_log(
        &log_sender.info_log_sender,
        format!(
            "Start download of remaining headers with node: {:?}\n",
            node.peer_addr()
        )
        .as_str(),
    );
    let mut first_block_found = false;
    request_headers_from_node(config, node, headers.clone())?;
    let mut headers_read = receive_headers_from_node(log_sender, node)?;

    load_header_heights(&headers_read, &header_heights, &headers)?;

    store_headers_in_local_headers_vec(log_sender, headers.clone(), &headers_read)?;
    while headers_read.len() == 2000 {
        request_headers_from_node(config, node, headers.clone())?;
        headers_read = receive_headers_from_node(log_sender, node)?;
        load_header_heights(&headers_read, &header_heights, &headers)?;
        store_headers_in_local_headers_vec(log_sender, headers.clone(), &headers_read)?;
        match first_block_found {
            true => {
                // If the first block has already been found, I send all the headers to the thread that downloads the blocks
                download_blocks_in_other_thread(tx.clone(), headers_read.clone())?;
            }
            false => {
                // If the first block has not been found, I check if it is in the headers I just received
                if first_block_to_download_is_in_headers(config, &headers_read)? {
                    // If the first block is in the headers I just received, I download the blocks that meet the configured date
                    download_first_blocks_in_other_thread(
                        config,
                        log_sender,
                        ui_sender,
                        headers_read.clone(),
                        tx.clone(),
                        &mut first_block_found,
                    )?;
                }
            }
        }
        let amount_of_headers = amount_of_headers(&headers)?;
        println!("{:?} headers downloaded", amount_of_headers - 1);
        send_event_to_ui(
            ui_sender,
            UIEvent::UpdateHeadersDownloaded(amount_of_headers - 1),
        );
    }
    Ok(())
}

/*
***************************************************************************
************************ AUXILIAR FUNCTIONS *******************************
***************************************************************************
*/

/// Checks for the last downloaded header and asks the node for the following headers with a getheaders message.
/// Returns an error if you cannot request them correctly or Ok(()) otherwise.
fn request_headers_from_node(
    config: &Arc<Config>,
    node: &mut TcpStream,
    headers: Arc<RwLock<Vec<BlockHeader>>>,
) -> Result<(), NodeCustomErrors> {
    let last_hash_header_downloaded: [u8; 32] = get_last_hash_header_downloaded(headers)?;
    GetHeadersMessage::build_getheaders_message(config, vec![last_hash_header_downloaded])
        .write_to(node)
        .map_err(|err| NodeCustomErrors::WriteNodeError(err.to_string()))?;
    Ok(())
}

/// Receives the headers from the node passed by parameter.
/// Returns a vector with the received headers or an error if you cannot receive them correctly.
pub fn receive_headers_from_node(
    log_sender: &LogSender,
    node: &mut TcpStream,
) -> Result<Vec<BlockHeader>, NodeCustomErrors> {
    let headers: Vec<BlockHeader> =
        HeadersMessage::read_from(log_sender, node, None).map_err(|_| {
            NodeCustomErrors::BlockchainDownloadError("Error trying to read headers".to_string())
        })?;
    Ok(headers)
}

/// Receives a vector of headers, validates them and saves them in the local headers vector.
/// If they are not valid, it does not save them and returns an error.
fn store_headers_in_local_headers_vec(
    log_sender: &LogSender,
    headers: Arc<RwLock<Vec<BlockHeader>>>,
    headers_read: &Vec<BlockHeader>,
) -> Result<(), NodeCustomErrors> {
    validate_headers(log_sender, headers_read)?;
    headers
        .write()
        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
        .extend_from_slice(headers_read);
    Ok(())
}

/// Receives a vector of headers that were downloaded in order (the first is more recent than the last)
/// and compares the timestamp of the last header of the vector with the time of the first block to download,
/// according to the configuration file. If the timestamp of the last header is greater than the time of the
/// first block to download, it returns true because the header of the block is within that vector. If not, it returns false.
fn first_block_to_download_is_in_headers(
    config: &Arc<Config>,
    headers: &[BlockHeader],
) -> Result<bool, NodeCustomErrors> {
    let block_timestamp = get_first_block_timestamp(config)?;
    let last_header = headers.last().ok_or(NodeCustomErrors::OtherError(
        "Can not get last header".to_string(),
    ))?;
    if last_header.time >= block_timestamp {
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Sends the first headers (after finding the first one to download) of those received by parameter that meet the date
/// established in the configuration file so that the respective blocks are downloaded. In case of an error when searching 
/// for the first header of the block to download, it returns an error, otherwise it returns Ok(()).
fn download_first_blocks_in_other_thread(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    headers_read: Vec<BlockHeader>,
    tx: Sender<Vec<BlockHeader>>,
    first_block_found: &mut bool,
) -> Result<(), NodeCustomErrors> {
    let first_block_headers_to_download =
        search_first_header_block_to_download(config, headers_read, first_block_found)
            .map_err(|err| NodeCustomErrors::FirstBlockNotFoundError(err.to_string()))?;
    write_in_log(
        &log_sender.info_log_sender,
        "First block to download found! Start blocks download\n",
    );
    send_event_to_ui(ui_sender, UIEvent::StartDownloadingBlocks);
    download_blocks_in_other_thread(tx, first_block_headers_to_download)?;
    Ok(())
}

/// Sens the headers received by parameter through the channel so that the respective blocks are 
/// downloaded in another thread. Returns an error if the channel is closed, otherwise Ok(()).
fn download_blocks_in_other_thread(
    tx: Sender<Vec<BlockHeader>>,
    headers_read: Vec<BlockHeader>,
) -> Result<(), NodeCustomErrors> {
    tx.send(headers_read)
        .map_err(|err| NodeCustomErrors::ThreadChannelError(err.to_string()))?;
    Ok(())
}

/// Returns the hash of the last downloaded header.
fn get_last_hash_header_downloaded(
    headers: Arc<RwLock<Vec<BlockHeader>>>,
) -> Result<[u8; 32], NodeCustomErrors> {
    let binding = headers
        .read()
        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?;
    let last_header = binding.last();
    match last_header {
        Some(header) => {
            if *header == GENESIS_BLOCK_HEADER {
                return Ok(GENESIS_BLOCK_HASH);
            }
            Ok(header.hash())
        }
        None => Err(NodeCustomErrors::BlockchainDownloadError(
            "Error, there are not headers downloaded!\n".to_string(),
        )),
    }
}

/// Validates that the header has the correct proof of work.
/// Returns an error if it is not valid or Ok(()) otherwise.
fn validate_headers(
    log_sender: &LogSender,
    headers: &Vec<BlockHeader>,
) -> Result<(), NodeCustomErrors> {
    for header in headers {
        if !header.validate() {
            write_in_log(
                &log_sender.error_log_sender,
                "Error in the validation of the header\n",
            );
            return Err(NodeCustomErrors::InvalidHeaderError(
                "partial validation of header is invalid!".to_string(),
            ));
        }
    }
    Ok(())
}

/// Recieves a vector of headers (in ascending order by timestamp) and returns
/// a vector of headers that have a timestamp greater than or equal to the first block that
/// is wanted to download (defined in configuration). In case it cannot obtain
/// the timestamp of the first block returns an error.
pub fn search_first_header_block_to_download(
    config: &Arc<Config>,
    headers: Vec<BlockHeader>,
    found: &mut bool,
) -> Result<Vec<BlockHeader>, NodeCustomErrors> {
    // get timestamp of the first block to download
    let timestamp = get_first_block_timestamp(config)?;
    let mut first_headers_from_blocks_to_download = vec![];
    for header in headers {
        // If it has not yet been found and the timestamp of the current header is greater 
        // than or equal to that of the first block to download
        if !(*found) && header.time >= timestamp {
            *found = true;
        }
        if *found {
            // If it has already been found, I add it (it is assumed that the headers are ordered by ascending timestamp)
            first_headers_from_blocks_to_download.push(header);
        }
    }
    Ok(first_headers_from_blocks_to_download)
}

/// Returns the timestamp of the first block to download.
/// If it cannot be obtained, it returns an error.
fn get_first_block_timestamp(config: &Config) -> Result<u32, NodeCustomErrors> {
    let date_time = Utc
        .datetime_from_str(
            &config.first_block_date,
            &config.date_format,
        )
        .map_err(|err| NodeCustomErrors::OtherError(err.to_string()))?;
    let timestamp = date_time.timestamp() as u32;
    Ok(timestamp)
}

/// Returns the amount of headers in the headers vector.
pub fn amount_of_headers(
    headers: &Arc<RwLock<Vec<BlockHeader>>>,
) -> Result<usize, NodeCustomErrors> {
    let amount_of_headers = headers
        .read()
        .map_err(|err| NodeCustomErrors::LockError(format!("{:?}", err)))?
        .len();
    Ok(amount_of_headers)
}

