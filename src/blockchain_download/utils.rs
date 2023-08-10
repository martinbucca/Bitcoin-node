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

/// Returns the last node of the list of connected nodes to download the headers of the blockchain.
/// If there are no more nodes available, it returns an error.
pub fn get_node(nodes: Arc<RwLock<Vec<TcpStream>>>) -> Result<TcpStream, NodeCustomErrors> {
    let node = nodes
        .write()
        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
        .pop();
    match node {
        Some(node) => Ok(node),
        None => Err(NodeCustomErrors::BlockchainDownloadError(
            "Error there are no more nodes available".to_string(),
        )),
    }
}

/// Adds the node received by parameter to the list of connected nodes.
/// Returns an error if you cannot access the list of nodes or Ok(()) if the node is added correctly.
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

/// Receives a vector of thread handles and waits for them all to finish, if any fails it returns an error.
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

/// Receives a pointer to a vector of headers and a pointer to a hashmap of blocks and returns the amount of headers and blocks in each one.
pub fn get_amount_of_headers_and_blocks(
    headers: &Arc<RwLock<Vec<BlockHeader>>>,
    blocks: &Arc<RwLock<HashMap<[u8; 32], Block>>>,
) -> Result<(usize, usize), NodeCustomErrors> {
    let amount_of_headers = amount_of_headers(headers)?;
    let amount_of_blocks = amount_of_blocks(blocks)?;
    Ok((amount_of_headers, amount_of_blocks))
}
