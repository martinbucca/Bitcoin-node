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

/// Almacena la blockchain y el utxo set. Mantiene referencias a las cuentas y los nodos conectados.
/// Inicializa también el NodeMessageHandler que es quien realiza la comunicación con los nodos.
#[derive(Debug, Clone)]
pub struct Node {
    pub connected_nodes: Arc<RwLock<Vec<TcpStream>>>,
    pub blockchain: Blockchain,
    pub accounts: Arc<RwLock<Arc<RwLock<Vec<Account>>>>>,
    pub peers_handler: NodeMessageHandler,
    pub node_pointers: NodeDataPointers,
}

impl Node {
    /// Inicializa el nodo. Recibe la blockchain ya descargada.
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
    /// Validar el bloque recibido
    pub fn block_validation(block: Block) -> (bool, &'static str) {
        block.validate()
    }

    /// Devuelve las utxos asociadas a la address recibida.
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
    /// Se encarga de llamar a la funcion finish() del peers_handler del nodo
    pub fn shutdown_node(&self) -> Result<(), NodeCustomErrors> {
        self.peers_handler.finish()
    }

    /// Recibe un vector de bytes que representa a la raw format transaction para se enviada por
    /// la red a todos los nodos conectados
    pub fn broadcast_tx(&self, raw_tx: [u8; 32]) -> Result<(), NodeCustomErrors> {
        let inventories = vec![Inventory::new_tx(raw_tx)];
        let inv_message_bytes = inv_mershalling(inventories);
        self.peers_handler.broadcast_to_nodes(inv_message_bytes)
    }

    /// Actualiza lo que apunta el puntero de accounts a otro puntero que es pasado por parametro
    /// de esta manera el puntero queda apuntando a un puntero con un vector de cuentas que es apuntado por la wallet
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

    /// Realiza la merkle proof of inclusion, delega la creacion del merkle tree al nodo, para
    /// que luego el merkle tree genere la proof of inclusion,devuelve error en caso de no encontrar
    /// el hash block, en caso de exito devuelve un option
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
                "No se encontro el bloque".to_string(),
            )),
        }
    }

    /// Se encarga de llamar a la funcion add_connection del peers_handler del nodo
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

    /// Busca un bloque en la blockchain
    /// Recibe el hash del bloque en formato hex
    /// Devuelve el bloque si lo encuentra, None en caso contrario
    pub fn search_block(&self, hash: [u8; 32]) -> Option<Block> {
        self.blockchain.search_block(hash)
    }

    /// Busca un header en la blockchain
    /// Recibe el hash del header en formato hex
    /// Devuelve el header si lo encuentra, None en caso contrario
    pub fn search_header(&self, hash: [u8; 32]) -> Option<(BlockHeader, usize)> {
        self.blockchain.search_header(hash)
    }
}
