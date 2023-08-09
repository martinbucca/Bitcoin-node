use gtk::glib;

use crate::{
    account::Account,
    blockchain::Blockchain,
    blocks::{block::Block, block_header::BlockHeader},
    custom_errors::NodeCustomErrors,
    gtk::ui_events::UIEvent,
    handler::node_message_handler::NodeMessageHandler,
    logwriter::log_writer::LogSender,
    messages::inventory::{inv_mershalling, Inventory},
    node_data_pointers::NodeDataPointers,
    utxo_tuple::UtxoTuple,
};
use std::{
    error::Error,
    net::TcpStream,
    sync::{Arc, RwLock},
};

type MerkleProofOfInclusionResult = Result<Option<Vec<([u8; 32], bool)>>, NodeCustomErrors>;

#[derive(Debug, Clone)]
/// Stores the blockchain and the utxo set. Keeps references to the accounts and the connected nodes.
/// It also initializes the NodeMessageHandler that is the one who communicates with the nodes.
pub struct Node {
    pub connected_nodes: Arc<RwLock<Vec<TcpStream>>>,
    pub blockchain: Blockchain,
    pub accounts: Arc<RwLock<Arc<RwLock<Vec<Account>>>>>,
    pub peers_handler: NodeMessageHandler,
    pub node_pointers: NodeDataPointers,
}

impl Node {
    /// Initializes the node. Receives the blockchain already downloaded.
    pub fn new(
        log_sender: &LogSender,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        connected_nodes: Arc<RwLock<Vec<TcpStream>>>,
        blockchain: Blockchain,
    ) -> Result<Self, NodeCustomErrors> {
        let pointer_to_accounts_in_node = Arc::new(RwLock::new(Arc::new(RwLock::new(vec![]))));
        let node_pointers = NodeDataPointers::new(
            connected_nodes.clone(),
            blockchain.clone(),
            pointer_to_accounts_in_node.clone(),
        );
        let peers_handler = NodeMessageHandler::new(log_sender, ui_sender, node_pointers.clone())?;
        Ok(Node {
            connected_nodes,
            blockchain,
            accounts: pointer_to_accounts_in_node,
            peers_handler,
            node_pointers,
        })
    }
    /// Validate the block received
    pub fn block_validation(block: Block) -> (bool, &'static str) {
        block.validate()
    }

    /// Returns the utxos associated with the received address.
    pub fn utxos_referenced_to_account(
        &self,
        address: &str,
    ) -> Result<Vec<UtxoTuple>, Box<dyn Error>> {
        let mut account_utxo_set: Vec<UtxoTuple> = Vec::new();
        for utxo in self
            .blockchain
            .utxo_set
            .read()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .values()
        {
            let aux_utxo = utxo.referenced_utxos(address);
            let utxo_to_push = match aux_utxo {
                Some(value) => value,
                None => continue,
            };
            account_utxo_set.push(utxo_to_push);
        }
        Ok(account_utxo_set)
    }

    /// Calls the finish() function of the node's peers_handler
    pub fn shutdown_node(&self) -> Result<(), NodeCustomErrors> {
        self.peers_handler.finish()
    }

    /// Receives a vec of bytes that represents the raw format transaction to be sent
    /// to all the connected nodes
    pub fn broadcast_tx(&self, raw_tx: [u8; 32]) -> Result<(), NodeCustomErrors> {
        let inventories = vec![Inventory::new_tx(raw_tx)];
        let inv_message_bytes = inv_mershalling(inventories);
        self.peers_handler.broadcast_to_nodes(inv_message_bytes)
    }

    /// Actualize what the accounts pointer points to another pointer that is passed by parameter
    /// in this way the pointer is pointing to a pointer with a vector of accounts that is pointed by the wallet
    pub fn set_accounts(
        &mut self,
        accounts: Arc<RwLock<Vec<Account>>>,
    ) -> Result<(), NodeCustomErrors> {
        *self
            .accounts
            .write()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))? = accounts;
        Ok(())
    }

    /// Makes the merkle proof of inclusion, delegates the creation of the merkle tree to the node, so
    /// that the merkle tree then generates the proof of inclusion, returns an error if the block hash is not found,
    /// in case of success it returns an option
    pub fn merkle_proof_of_inclusion(
        &self,
        block_hash: &[u8; 32],
        tx_hash: &[u8; 32],
    ) -> MerkleProofOfInclusionResult {
        let block_chain = self
            .blockchain
            .blocks
            .read()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?;
        let block_option = block_chain.get(block_hash);

        match block_option {
            Some(block) => Ok(block.merkle_proof_of_inclusion(tx_hash)),
            None => Err(NodeCustomErrors::OtherError(
                "Block hash not found".to_string(),
            )),
        }
    }

    /// Calls the add_connection function of the node's peers_handler
    pub fn add_connection(
        &mut self,
        log_sender: &LogSender,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        connection: TcpStream,
    ) -> Result<(), NodeCustomErrors> {
        self.peers_handler.add_connection(
            log_sender,
            ui_sender,
            self.node_pointers.clone(),
            connection,
        )
    }

    /// Searchs a block in the blockchain.
    /// Receives the hash of the block in hex format.
    /// Returns the block if it finds it, None otherwise.
    pub fn search_block(&self, hash: [u8; 32]) -> Option<Block> {
        self.blockchain.search_block(hash)
    }

    /// Searchs a header in the blockchain.
    /// Receives the hash of the header in hex format.
    /// Returns the header if it finds it, None otherwise.
    pub fn search_header(&self, hash: [u8; 32]) -> Option<(BlockHeader, usize)> {
        self.blockchain.search_header(hash)
    }
}
