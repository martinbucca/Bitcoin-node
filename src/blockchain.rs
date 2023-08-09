use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    blocks::{block::Block, block_header::BlockHeader},
    utxo_tuple::UtxoTuple,
};
type UtxoSetPointer = Arc<RwLock<HashMap<[u8; 32], UtxoTuple>>>;

#[derive(Debug, Clone)]
/// Represents the blockchain with its blocks, headers, heights and UTXO set.
pub struct Blockchain {
    pub headers: Arc<RwLock<Vec<BlockHeader>>>,
    pub blocks: Arc<RwLock<HashMap<[u8; 32], Block>>>,
    pub header_heights: Arc<RwLock<HashMap<[u8; 32], usize>>>,
    pub utxo_set: UtxoSetPointer,
}

impl Blockchain {
    /// Creates a new Blockchain that groups the headers, blocks, heights and UTXO set.
    pub fn new(
        headers: Arc<RwLock<Vec<BlockHeader>>>,
        blocks: Arc<RwLock<HashMap<[u8; 32], Block>>>,
        header_heights: Arc<RwLock<HashMap<[u8; 32], usize>>>,
        utxo_set: UtxoSetPointer,
    ) -> Self {
        Blockchain {
            headers,
            blocks,
            header_heights,
            utxo_set,
        }
    }

    /// Searchs a block in the blockchain.
    /// Receives the hash of the block in hex format.
    /// Returns the block if it finds it, None if it can't get the lock or if it doesn't find it.
    pub fn search_block(&self, hash: [u8; 32]) -> Option<Block> {
        if let Ok(blocks) = self.blocks.read() {
            return blocks.get(&hash).cloned();
        } else {
            None
        }
    }

    /// Searchs a header in the blockchain.
    /// Receives the hash of the header in hex format.
    /// Returns the header if it finds it, None if it can't get the lock or if it doesn't find it.
    pub fn search_header(&self, hash: [u8; 32]) -> Option<(BlockHeader, usize)> {
        if let Ok(index) = self.header_heights.read() {
            if let Some(height) = index.get(&hash) {
                if let Ok(headers) = self.headers.read() {
                    if let Some(header) = headers.get(*height).cloned() {
                        return Some((header, *height));
                    }
                }
            }
        }
        None
    }
}
