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
/// Representa la cadena de bloques con sus bloques, headers, alturas y UTXO set.
pub struct Blockchain {
    pub headers: Arc<RwLock<Vec<BlockHeader>>>,
    pub blocks: Arc<RwLock<HashMap<[u8; 32], Block>>>,
    pub header_heights: Arc<RwLock<HashMap<[u8; 32], usize>>>,
    pub utxo_set: UtxoSetPointer,
}

impl Blockchain {
    /// Crea un nuevo Blockchain que agrupa los headers, bloques, alturas y UTXO set.
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

    /// Busca un bloque en la blockchain
    /// Recibe el hash del bloque en formato hex
    /// Devuelve el bloque si lo encuentra, None en caso de error al obtener el lock o no encontrarlo
    pub fn search_block(&self, hash: [u8; 32]) -> Option<Block> {
        if let Ok(blocks) = self.blocks.read() {
            return blocks.get(&hash).cloned();
        } else {
            None
        }
    }

    /// Busca un header en la blockchain
    /// Recibe el hash del header en formato hex
    /// Devuelve el header si lo encuentra, None en caso de error al obtener el lock o no encontrarlo
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
