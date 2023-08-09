use std::{
    net::TcpStream,
    sync::{Arc, RwLock},
};

use crate::{account::Account, blockchain::Blockchain};

/// Almacena los punteros de los datos del nodo que se comparten entre los hilos.
#[derive(Debug, Clone)]
pub struct NodeDataPointers {
    pub connected_nodes: Arc<RwLock<Vec<TcpStream>>>,
    pub blockchain: Blockchain,
    pub accounts: Arc<RwLock<Arc<RwLock<Vec<Account>>>>>,
}

impl NodeDataPointers {
    /// Almacena los punteros de los datos del nodo que se comparten entre los hilos.
    pub fn new(
        connected_nodes: Arc<RwLock<Vec<TcpStream>>>,
        blockchain: Blockchain,
        accounts: Arc<RwLock<Arc<RwLock<Vec<Account>>>>>,
    ) -> Self {
        NodeDataPointers {
            connected_nodes,
            blockchain,
            accounts,
        }
    }
}
