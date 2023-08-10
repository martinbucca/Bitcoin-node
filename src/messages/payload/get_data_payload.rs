use std::error::Error;

use crate::{compact_size_uint::CompactSizeUint, messages::inventory::Inventory};

const INV_SIZE: usize = 36;

#[derive(Debug)]
/// Represents the getdata message of the bitcoin protocol.
/// It transmits one or more inventories (hashes).
/// It can be the response to the getdata message.
pub struct GetDataPayload {
    pub count: CompactSizeUint,
    pub inventories: Vec<Inventory>,
    get_data_payload_bytes: Vec<u8>,
}

impl GetDataPayload {
    /// Given a vector of inventory, it returns the payload of the getdata message.
    pub fn get_payload(inventories: Vec<Inventory>) -> GetDataPayload {
        let count = CompactSizeUint::new(inventories.len() as u128);
        let get_data_payload_bytes = get_data_payload_bytes(&count, &inventories);
        GetDataPayload {
            count,
            inventories,
            get_data_payload_bytes,
        }
    }

    /// Returns a vector of bytes representing the payload of the getdata message.
    pub fn to_le_bytes(&self) -> &[u8] {
        &self.get_data_payload_bytes
    }

    /// Returns the size in bytes of the payload.
    pub fn size(&self) -> usize {
        self.to_le_bytes().len()
    }
}

/// Returns the payload serialized to bytes.
fn get_data_payload_bytes(count: &CompactSizeUint, inventories: &Vec<Inventory>) -> Vec<u8> {
    let mut getdata_payload_bytes: Vec<u8> = vec![];
    getdata_payload_bytes.extend_from_slice(&count.marshalling());
    for inventory in inventories {
        getdata_payload_bytes.extend(inventory.to_le_bytes());
    }
    getdata_payload_bytes
}

/// Receives the payload of the getdata message in a byte string and returns a vector of Inventory.
pub fn unmarshalling(payload: &[u8]) -> Result<Vec<Inventory>, Box<dyn Error>> {
    let mut offset: usize = 0;
    let count = CompactSizeUint::unmarshalling(payload, &mut offset)?;
    let mut inventories: Vec<Inventory> = Vec::new();
    for _ in 0..count.decoded_value() as usize {
        let mut inventory_bytes = vec![0; INV_SIZE];
        inventory_bytes.copy_from_slice(&payload[offset..(offset + INV_SIZE)]);
        let inv = Inventory::from_le_bytes(&inventory_bytes);
        inventories.push(inv);
        offset += INV_SIZE; // szie of inventory
    }
    Ok(inventories)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_with_one_inventory_is_created_correctly() {
        // GIVEN: an inventory with a single hash
        let mut inventories = Vec::new();
        inventories.push(Inventory::new_block([0; 32]));
        // WHEN: the get_payload method is called
        let payload = GetDataPayload::get_payload(inventories.clone());
        // THEN: the attributes of GetDataPayload were created correctly.
        assert_eq!(payload.count.decoded_value() as usize, inventories.len());
    }

    #[test]
    fn payload_with_two_inventory_is_created_correctly() {
        // GIVEN: an inventory with two hashes
        let mut inventories = Vec::new();
        inventories.push(Inventory::new_block([0; 32]));
        inventories.push(Inventory::new_block([0; 32]));
        // WHEN: the get_payload method is called
        let payload = GetDataPayload::get_payload(inventories.clone());
        // THEN: the attributes of GetDataPayload were created correctly.
        assert_eq!(payload.count.decoded_value() as usize, inventories.len());
    }
}
