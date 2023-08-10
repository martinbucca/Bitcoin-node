use bitcoin_hashes::{sha256d, Hash};
use std::io::Write;

use super::{
    inventory::Inventory, message_header::HeaderMessage, payload::get_data_payload::GetDataPayload,
};

const START_STRING: [u8; 4] = [0x0b, 0x11, 0x09, 0x07];

#[derive(Debug)]
/// Implemnts the getdata message needed to request objects from another node.
/// It can be used to request transactions, blocks, etc.
/// The payload is similar to the Inv message.
pub struct GetDataMessage {
    pub header: HeaderMessage,
    pub payload: GetDataPayload,
}
impl GetDataMessage {
    /// Creates a new getdata message from the inventories,
    /// which are the hashes of some object, such as tx or block
    /// # EXAMPLE of use:
    ///```no_test
    ///     let hash:[u8;32] = [
    ///         0x56, 0x48, 0x22, 0x54,
    ///         0x8a, 0x41, 0x0e, 0x1d,
    ///         0xcf, 0xa0, 0xc7, 0x21,
    ///         0x90, 0xb7, 0x28, 0xd4,
    ///         0xc2, 0x93, 0xc3, 0x14,
    ///         0xb6, 0xf2, 0x2b, 0x16,
    ///         0x13, 0x00, 0x00, 0x00,
    ///         0x00, 0x00, 0x00, 0x00
    ///     ];
    ///     let mut inventories = Vec::new();
    ///     inventories.push(Inventory::new_block(hash));
    ///
    ///     let data_message = GetDataMessage::new(inventories);
    ///     data_message.write_to(&mut stream);
    /// ```
    pub fn new(inventories: Vec<Inventory>) -> GetDataMessage {
        let payload = GetDataPayload::get_payload(inventories);
        let header = get_data_header_message(&payload);
        GetDataMessage { header, payload }
    }

    /// Marshalls the get_data message and returns the array of bytes to be written on the network.
    pub fn marshalling(&self) -> Vec<u8> {
        let header = self.header.to_le_bytes();
        let payload = self.payload.to_le_bytes();
        let mut get_data_bytes: Vec<u8> = Vec::new();
        get_data_bytes.extend_from_slice(&header);
        get_data_bytes.extend(payload);
        get_data_bytes
    }
    /// Given a GetHeadersMessage struct and a stream that implements the Write trait where it can be written,
    /// writes the serialized message to bytes in the stream and returns an Ok () if it could be written correctly,
    /// and an error if it was not written correctly in the stream.
    pub fn write_to(&self, stream: &mut dyn Write) -> std::io::Result<()> {
        let message = self.marshalling();
        stream.write_all(&message)?;
        stream.flush()?;
        Ok(())
    }
}

/// Returns the Header Message of the getdata message.
fn get_data_header_message(payload: &GetDataPayload) -> HeaderMessage {
    let payload_bytes = payload.to_le_bytes();
    let binding = sha256d::Hash::hash(payload_bytes);
    let checksum = binding.as_byte_array();
    HeaderMessage {
        start_string: START_STRING,
        command_name: "getdata".to_string(),
        payload_size: payload_bytes.len() as u32,
        checksum: [checksum[0], checksum[1], checksum[2], checksum[3]],
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_data_message_with_one_inventory_is_created_with_correct_command_name() {
        // GIVEN: an inventory with a single null hash
        let mut inventories = Vec::new();
        inventories.push(Inventory::new_block([0; 32]));
        // WHEN: the get_payload method is called
        let message = GetDataMessage::new(inventories);
        // THEN: the message header is created with the correct command_name.
        assert!(message.header.command_name.contains("getdata"));
    }
}

