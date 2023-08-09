use std::{
    net::TcpStream,
    sync::{Arc, RwLock},
};

use crate::{account::Account, blockchain::Blockchain};

#[derive(Debug, Clone)]
/// Stores the pointers of the node data that are shared between threads.
pub struct NodeDataPointers {
    pub connected_nodes: Arc<RwLock<Vec<TcpStream>>>,
    pub blockchain: Blockchain,
    pub accounts: Arc<RwLock<Arc<RwLock<Vec<Account>>>>>,
}

impl NodeDataPointers {
    /// Returns a new instance of NodeDataPointers
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
