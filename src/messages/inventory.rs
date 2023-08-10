use crate::compact_size_uint::CompactSizeUint;

use super::message_header::HeaderMessage;

#[derive(Debug, Clone)]
/// Represents an inventory of the bitcoin protocol.
/// the type_identifier indicates what the hash corresponds to:
/// block, transaction, etc.
pub struct Inventory {
    pub type_identifier: u32,
    pub hash: [u8; 32],
}

impl Inventory {
    /// Creates an inventory with the hash of a block.
    pub fn new_block(hash: [u8; 32]) -> Inventory {
        Inventory {
            type_identifier: 2, // 2: Block
            hash,
        }
    }

    /// Creates an inventory with the hash of a transaction.
    pub fn new_tx(hash: [u8; 32]) -> Inventory {
        Inventory {
            type_identifier: 1, // 1: Transaction
            hash,
        }
    }

    /// Converts the Inventory to little endian bytes, as required by the bitcoin protocol
    /// to send it over the network.
    pub fn to_le_bytes(&self) -> Vec<u8> {
        let mut inventory_bytes: Vec<u8> = Vec::new();
        inventory_bytes.extend_from_slice(&self.type_identifier.to_le_bytes());
        inventory_bytes.extend(self.hash);
        inventory_bytes
    }

    /// Receives a byte string, deserializes it and returns the Inventory.
    pub fn from_le_bytes(inventory_bytes: &[u8]) -> Inventory {
        let mut type_identifier_bytes = [0; 4];
        type_identifier_bytes.copy_from_slice(&inventory_bytes[0..4]);
        let mut hash_bytes = [0; 32];
        hash_bytes.copy_from_slice(&inventory_bytes[4..36]);
        Inventory {
            type_identifier: u32::from_le_bytes(type_identifier_bytes),
            hash: hash_bytes,
        }
    }

    /// Returns the hash contained in the inventory.
    pub fn hash(&self) -> [u8; 32] {
        self.hash
    }
}

/// Receives a vector of Inventory and serializes the inv message with that vector. Returns a vector
/// of u8 representing the serialized bytes.
pub fn inv_mershalling(inventories: Vec<Inventory>) -> Vec<u8> {
    let count = CompactSizeUint::new(inventories.len() as u128);
    let mut inv_payload = vec![];
    inv_payload.extend_from_slice(&count.marshalling());
    for inventory in inventories {
        inv_payload.extend(inventory.to_le_bytes());
    }
    let header = HeaderMessage::new("inv".to_string(), Some(&inv_payload));
    let mut inv_message = vec![];
    inv_message.extend_from_slice(&header.to_le_bytes());
    inv_message.extend_from_slice(&inv_payload);
    inv_message
}
