use super::{
    inventory::{inv_mershalling, Inventory},
    message_header::HeaderMessage,
};

/// Receives a vector of Inventory and returns the serialized notfound message.
pub fn get_notfound_message(inventories: Vec<Inventory>) -> Vec<u8> {
    let mut message = vec![];
    let payload = inv_mershalling(inventories);
    let header = HeaderMessage::new("notfound".to_string(), Some(&payload));
    message.extend_from_slice(&header.to_le_bytes());
    message.extend_from_slice(&payload);
    message
}
