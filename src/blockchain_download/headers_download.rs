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

/// Descarga los primeros headers de la blockchain y los guarda en disco
/// En caso de ya estar guardados, los lee desde ahi, en caso contrario
/// los lee y los guarda
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
            write_in_log(
                &log_sender.error_log_sender,
                format!("Error al leer headers de disco: {}", err).as_str(),
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
            "Empiezo lectura de los primeros {} headers de disco",
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
        println!("{:?} headers leidos", amount);
        send_event_to_ui(
            ui_sender,
            UIEvent::ActualizeHeadersDownloaded(amount as usize),
        );
        i += HEADERS_MESSAGE_SIZE;
    }
    write_in_log(
        &log_sender.info_log_sender,
        format!("Se leyeron correctamente {:?} headers de disco", amount).as_str(),
    );
    Ok(())
}

/// Carga los hashes de los headers en un hashmap para poder obtener la altura de un header en O(1)
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
            "Empiezo descarga de los primeros {} headers para guardarlos en disco",
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
                "Fallo la descarga con el nodo --{:?}--, lo descarto y voy a intentar con otro. Error: {}",
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

/// Descarga los primeros headers (especificados en el archivo de configuracion) desde un nodo de la blockchain y los guarda en disco
/// El genesis block no lo descarga de la red, ya lo tiene hardcodeado
/// Devuelve un error en caso de no poder descargar los headers exitosamente.
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
            "Empiezo descarga de headers con nodo: {:?}\n",
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
            "{:?} headers descargados y guardados en disco",
            amount_of_headers
        );
        send_event_to_ui(
            ui_sender,
            UIEvent::ActualizeHeadersDownloaded(amount_of_headers),
        );
    }
    Ok(())
}

/// Recibe los headers del nodo y los guarda en disco
/// Devuelve un error en caso de no poder recibirlos correctamente
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
            "Error al leer y persistir headers iniciales".to_string(),
        )
    })?;
    Ok(headers)
}

/*
***************************************************************************
******************* DOWNLOAD HEADERS FROM NETWORK *************************
***************************************************************************
*/

/// Descarga los headers de la blockchain desde los nodos conectados
/// En caso de que un nodo falle en la descarga, intenta con otro siempre y cuando tenga peers disponibles
/// Devuelve un error en caso de no poder descargar los headers desde nignun nodo peer
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
                "Fallo la descarga con el nodo --{:?}--, lo descarto y voy a intentar con otro. Error: {}",
                node.peer_addr(),
                err
            )
            .as_str(),
        );
        if let NodeCustomErrors::ThreadChannelError(_) = err {
            return Err(NodeCustomErrors::ThreadChannelError("Error se cerro el channel que comunica la descarga de headers y bloques en paralelo".to_string()));
        }
        node = get_node(nodes.clone())?;
    }
    // return node again to the list of nodes
    return_node_to_vec(nodes, node)?;
    /*
    let last_headers =
        compare_and_ask_for_last_headers(config, log_sender, ui_sender, nodes, headers.clone(), header_heights)?;
    if !last_headers.is_empty() {
        write_in_log(
            &log_sender.info_log_sender,
            format!(
                "Agrego ultimos {} headers enocontrados al comparar con todos los nodos",
                last_headers.len()
            )
            .as_str(),
        );
        tx.send(last_headers)
            .map_err(|err| NodeCustomErrors::ThreadChannelError(err.to_string()))?;
    }
    */
    send_event_to_ui(
        ui_sender,
        UIEvent::FinsihDownloadingHeaders(amount_of_headers(&headers)?),
    );
    Ok(())
}

/// Descarga los headers de un nodo en particular y los guarda en el vector de headers
/// En caso de que el parametro tx sea un Sender, envia los headers que va descargando al thread
/// que descarga bloques para que se descarguen en paralelo, en caso contrario no envia nada.
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
            "Empiezo la descarga de todos los headers que faltan con nodo: {:?}\n",
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
                // si el primer bloque ya fue encontrado, envio al thread de descarga de bloques todos los headers
                download_blocks_in_other_thread(tx.clone(), headers_read.clone())?;
            }
            false => {
                // si el primer bloque no fue encontrado, me fijo si esta en los headers que acabo de recibir
                if first_block_to_download_is_in_headers(config, &headers_read)? {
                    // si el primer bloque esta en los headers que acabo de recibir, descargo los bloques que cumplan con la fecha configurada
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
        println!("{:?} headers descargados", amount_of_headers - 1);
        send_event_to_ui(
            ui_sender,
            UIEvent::ActualizeHeadersDownloaded(amount_of_headers - 1),
        );
    }
    Ok(())
}

/*
***************************************************************************
************************ AUXILIAR FUNCTIONS *******************************
***************************************************************************
*/

/// Se fija por el ultimo header descargado y pide al nodo los headers siguientes con un mensaje getheaders
/// Devuelve un error en caso de no poder pedirlos correctamente
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

/// Recibe el headers del nodo pasado por parametro.
/// Devuelve un vector con los headers recibidos o error en caso de no poder recibirlos correctamente.
pub fn receive_headers_from_node(
    log_sender: &LogSender,
    node: &mut TcpStream,
) -> Result<Vec<BlockHeader>, NodeCustomErrors> {
    let headers: Vec<BlockHeader> =
        HeadersMessage::read_from(log_sender, node, None).map_err(|_| {
            NodeCustomErrors::BlockchainDownloadError("Error al leer headers".to_string())
        })?;
    Ok(headers)
}

/// Recibe un vector de headers, los valida y los guarda en el vector de headers local
/// en caso de que no sean validos no los guarda y devuelve un error
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

/// Recibe un vector de headers que fueron descargados en orden (es decir que el primero es mas reciente que el ultimo)
/// y compara el timestamp del ultimo header del vector con el tiempo del primer bloque a descargar, segun el archivo de
/// configuracion. Si el timestamp del ultimo header es mayor al tiempo del primer bloque a descargar, devuelve true
/// porque el header del bloque se encuentra dentro de ese vector. Si no, devuelve false.
fn first_block_to_download_is_in_headers(
    config: &Arc<Config>,
    headers: &[BlockHeader],
) -> Result<bool, NodeCustomErrors> {
    let block_timestamp = get_first_block_timestamp(config)?;
    let last_header = headers.last().ok_or(NodeCustomErrors::OtherError(
        "No puedo obtener ulitmo header".to_string(),
    ))?;
    if last_header.time >= block_timestamp {
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Envia por el channel los primeros headers (luego de encontrar el primero a descargar) de los recibidos por parametro que cumplan con la fecha
/// establecida en el archivo de configuracion para que los respectivos bloques sean descargados.
/// En caso de un error al buscar el primer header del bloque a descargar devuelve un error
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
        "Encontre primer bloque a descargar! Empieza descarga de bloques\n",
    );
    send_event_to_ui(ui_sender, UIEvent::StartDownloadingBlocks);
    download_blocks_in_other_thread(tx, first_block_headers_to_download)?;
    Ok(())
}

/// Envia por el channel los headers recibidos por parametro para que los respectivos bloques sean descargados en otro thread
/// Devuelve error en caso de que el channel este cerrado
fn download_blocks_in_other_thread(
    tx: Sender<Vec<BlockHeader>>,
    headers_read: Vec<BlockHeader>,
) -> Result<(), NodeCustomErrors> {
    tx.send(headers_read)
        .map_err(|err| NodeCustomErrors::ThreadChannelError(err.to_string()))?;
    Ok(())
}

/// Devuelve el hash del ultimo header descargado
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
            "Error no hay headers descargados!\n".to_string(),
        )),
    }
}

/// Valida que el header tenga la proof of work correcta
/// Devuelve un error en caso de que no sea valido
fn validate_headers(
    log_sender: &LogSender,
    headers: &Vec<BlockHeader>,
) -> Result<(), NodeCustomErrors> {
    for header in headers {
        if !header.validate() {
            write_in_log(
                &log_sender.error_log_sender,
                "Error en validacion de la proof of work de header",
            );
            return Err(NodeCustomErrors::InvalidHeaderError(
                "partial validation of header is invalid!".to_string(),
            ));
        }
    }
    Ok(())
}

/// Recorre un vector de headers (en orden ascendente por timestamp) y devuelve
/// un vector de headers que tienen timestamp mayor o igual al del primer bloque que
/// se quiere descargar (definido en configuracion). En caso de no poder obtener
/// el timestamp del primer bloque devuelve un error
pub fn search_first_header_block_to_download(
    config: &Arc<Config>,
    headers: Vec<BlockHeader>,
    found: &mut bool,
) -> Result<Vec<BlockHeader>, NodeCustomErrors> {
    // obtengo timestampo del primer bloque que se quiere descargar
    let timestamp = get_first_block_timestamp(config)?;
    let mut first_headers_from_blocks_to_download = vec![];
    for header in headers {
        // si aun no fue encontrado y el timestampo del header actual es mayor o igual que el del primer bloque a descargar
        if !(*found) && header.time >= timestamp {
            *found = true;
        }
        if *found {
            // si ya fue encontrado, lo agrego (se asume que los headers estan ordenados por timestamp ascendente)
            first_headers_from_blocks_to_download.push(header);
        }
    }
    Ok(first_headers_from_blocks_to_download)
}

/// Devuelve el timestamp del primer bloque a descargar.
/// En caso de no poder obtenerlo devuelve un error
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

/// Devuelve la cantidad de headers que hay en el vector de headers
pub fn amount_of_headers(
    headers: &Arc<RwLock<Vec<BlockHeader>>>,
) -> Result<usize, NodeCustomErrors> {
    let amount_of_headers = headers
        .read()
        .map_err(|err| NodeCustomErrors::LockError(format!("{:?}", err)))?
        .len();
    Ok(amount_of_headers)
}

/*
/// Once the headers are downloaded, this function recieves the nodes and headers  downloaded
/// and sends a getheaders message to each node to compare and get a header that was not downloaded.
/// it returns error in case of failure.
fn compare_and_ask_for_last_headers(
    config: &Arc<Config>,
    log_sender: &LogSender,
    ui_sender: &Option<glib::Sender<UIEvent>>,
    nodes: Arc<RwLock<Vec<TcpStream>>>,
    headers: Arc<RwLock<Vec<BlockHeader>>>,
    header_heights: Arc<RwLock<HashMap<[u8; 32], usize>>>,
) -> Result<Vec<BlockHeader>, NodeCustomErrors> {
    // voy guardando los nodos que saco aca para despues agregarlos al puntero
    let mut nodes_vec: Vec<TcpStream> = vec![];
    let mut new_headers = vec![];
    // recorro todos los nodos
    while !nodes
        .read()
        .map_err(|err| NodeCustomErrors::LockError(format!("{:?}", err)))?
        .is_empty()
    {
        let mut node = nodes
            .write()
            .map_err(|err| NodeCustomErrors::LockError(format!("{:?}", err)))?
            .pop()
            .ok_or("Error no hay mas nodos para comparar y descargar ultimos headers!\n")
            .map_err(|err| NodeCustomErrors::CanNotRead(err.to_string()))?;
        let last_header = headers
            .read()
            .map_err(|err| NodeCustomErrors::LockError(format!("{:?}", err)))?
            .last()
            .ok_or("Error no hay headers guardados, no tengo para comparar...\n")
            .map_err(|err| NodeCustomErrors::CanNotRead(err.to_string()))?
            .hash();
        GetHeadersMessage::build_getheaders_message(config, vec![last_header])
            .write_to(&mut node)
            .map_err(|err| NodeCustomErrors::WriteNodeError(err.to_string()))?;
        let headers_read = match HeadersMessage::read_from(log_sender, &mut node, None) {
            Ok(headers) => headers,
            Err(err) => {
                write_in_log(
                    &log_sender.error_log_sender,
                    format!("Error al tratar de leer nuevos headers, descarto nodo. Error: {err}")
                        .as_str(),
                );
                continue;
            }
        };
        // si se recibio un header nuevo lo agrego
        if !headers_read.is_empty() {
            headers
                .write()
                .map_err(|err| NodeCustomErrors::LockError(format!("{:?}", err)))?
                .extend_from_slice(&headers_read);
            write_in_log(
                &log_sender.info_log_sender,
                format!(
                    "{} headers encontrados al comparar el ultimo mio con el nodo: {:?}",
                    headers_read.len(),
                    node
                )
                .as_str(),
            );
            new_headers.extend_from_slice(&headers_read);
        }
        nodes_vec.push(node);
    }
    // devuelvo todos los nodos a su puntero
    nodes
        .write()
        .map_err(|err| NodeCustomErrors::LockError(format!("{:?}", err)))?
        .extend(nodes_vec);
    Ok(new_headers)
}
*/
