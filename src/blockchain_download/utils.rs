use crate::{
    blocks::{block::Block, block_header::BlockHeader},
    custom_errors::NodeCustomErrors,
};
use std::{
    collections::HashMap,
    net::TcpStream,
    sync::{Arc, RwLock},
    thread,
};

use super::{blocks_download::amount_of_blocks, headers_download::amount_of_headers};

/// Devuelve el ultimo nodo de la lista de nodos conectados para descargar los headers de la blockchain
/// En caso de no haber mas nodos disponibles devuelve un error
pub fn get_node(nodes: Arc<RwLock<Vec<TcpStream>>>) -> Result<TcpStream, NodeCustomErrors> {
    let node = nodes
        .write()
        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
        .pop();
    match node {
        Some(node) => Ok(node),
        None => Err(NodeCustomErrors::BlockchainDownloadError(
            "Error no hay mas nodos conectados para descargar los headers de la blockchain!\n"
                .to_string(),
        )),
    }
}

/// Agrega el nodo recibido por parametro a la lista de nodos conectados
/// Devuelve error en caso de no poder acceder al vector de nodos.
/// Ok en caso contrario
pub fn return_node_to_vec(
    nodes: Arc<RwLock<Vec<TcpStream>>>,
    node: TcpStream,
) -> Result<(), NodeCustomErrors> {
    nodes
        .write()
        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
        .push(node);
    Ok(())
}

/// Recibe un vector de handles de threads y espera a que terminen todos, si alguno falla devuelve error
pub fn join_threads(
    handles: Vec<thread::JoinHandle<Result<(), NodeCustomErrors>>>,
) -> Result<(), NodeCustomErrors> {
    for handle in handles {
        handle
            .join()
            .map_err(|err| NodeCustomErrors::ThreadJoinError(format!("{:?}", err)))??;
    }
    Ok(())
}

/// Recibe un puntero a un vector de headers y un puntero a un hashmap de bloques y devuelve la cantidad de headers y bloques que hay en cada uno
pub fn get_amount_of_headers_and_blocks(
    headers: &Arc<RwLock<Vec<BlockHeader>>>,
    blocks: &Arc<RwLock<HashMap<[u8; 32], Block>>>,
) -> Result<(usize, usize), NodeCustomErrors> {
    let amount_of_headers = amount_of_headers(headers)?;
    let amount_of_blocks = amount_of_blocks(blocks)?;
    Ok((amount_of_headers, amount_of_blocks))
}
