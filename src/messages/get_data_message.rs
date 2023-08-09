use bitcoin_hashes::{sha256d, Hash};
use std::io::Write;

use super::{
    inventory::Inventory, message_header::HeaderMessage, payload::get_data_payload::GetDataPayload,
};

const START_STRING: [u8; 4] = [0x0b, 0x11, 0x09, 0x07];

// todo: el write_to es código repetido, es igual que el de getheaders_message.rs. Habría que extraerlos.
/// Implementa el mensaje getdata necesario para solicitar objetos a otro nodo.
/// Puede usarse para solicitar transacciones, bloques, etc.
/// El payload es similar al del mensaje Inv.
#[derive(Debug)]

pub struct GetDataMessage {
    pub header: HeaderMessage,
    pub payload: GetDataPayload,
}
impl GetDataMessage {
    /// Crea el mensaje getdata a partir de los inventories,
    /// los cuales son los hashes de algún objeto, tal como tx o block
    ///
    /// # EJEMPLO de uso:
    /// ```no_test
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
    ///
    pub fn new(inventories: Vec<Inventory>) -> GetDataMessage {
        let payload = GetDataPayload::get_payload(inventories);
        let header = get_data_header_message(&payload);
        GetDataMessage { header, payload }
    }

    /// Serializa el mensaje get_data y devuelve el array de bytes para ser escrito en la red
    pub fn marshalling(&self) -> Vec<u8> {
        let header = self.header.to_le_bytes();
        let payload = self.payload.to_le_bytes();
        let mut get_data_bytes: Vec<u8> = Vec::new();
        get_data_bytes.extend_from_slice(&header);
        get_data_bytes.extend(payload);
        get_data_bytes
    }
    /// Dado un struct GetHeadersMessage y un stream que implemente el trait Write en donde se pueda escribir,
    /// escribe el mensaje serializado a bytes en el stream y devuelve un Ok() si lo pudo escribir correctamente,
    /// y un error si no se escribio correctamente en el stream
    pub fn write_to(&self, stream: &mut dyn Write) -> std::io::Result<()> {
        let message = self.marshalling();
        stream.write_all(&message)?;
        stream.flush()?;
        Ok(())
    }
}

/// Devuelve el Header Message del mensaje getdata.
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
    fn get_data_message_con_un_inventory_se_crea_con_el_command_name_correcto() {
        // GIVEN : un inventory con un solo hash nulo
        let mut inventories = Vec::new();
        inventories.push(Inventory::new_block([0; 32]));
        // WHEN: se llama al método get_payload
        let message = GetDataMessage::new(inventories);
        // THEN: el header del mensaje se creó con el command_name correcto.
        assert!(message.header.command_name.contains("getdata"));
    }
}
