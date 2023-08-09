use std::{error::Error, io::Read, net::TcpStream};

use crate::{blocks::block::Block, logwriter::log_writer::LogSender};

use super::message_header::HeaderMessage;

/// Representa el mensaje block que se recibe en respuesta al mensaje getdata
#[derive(Debug)]
pub struct BlockMessage;

impl BlockMessage {
    /// Recibe en bytes el mensaje "block".
    /// Devuelve un bloque
    pub fn unmarshalling(block_message_payload_bytes: &Vec<u8>) -> Result<Block, Box<dyn Error>> {
        let mut offset = 0;
        let block = Block::unmarshalling(block_message_payload_bytes, &mut offset)?;
        Ok(block)
    }
    /// Dado un stream que implementa el trait Read (desde donde se puede leer) lee el mensaje block y devuelve
    /// el bloque correspondiente si se pudo leer correctamente o un Error en caso contrario.
    pub fn read_from(
        log_sender: &LogSender,
        stream: &mut TcpStream,
    ) -> Result<Block, Box<dyn std::error::Error>> {
        let header = HeaderMessage::read_from(log_sender, stream, "block".to_string(), None)?;
        let payload_size = header.payload_size as usize;
        let mut buffer_num = vec![0; payload_size];
        stream.read_exact(&mut buffer_num)?;
        let mut block_message_payload_bytes: Vec<u8> = vec![];
        block_message_payload_bytes.extend_from_slice(&buffer_num);
        let block = Self::unmarshalling(&block_message_payload_bytes)?;
        Ok(block)
    }
}

// Devuelve el mensaje de tipo block con el bloque pasado por parametro
pub fn get_block_message(block: &Block) -> Vec<u8> {
    let mut block_payload = vec![];
    block.marshalling(&mut block_payload);
    let header = HeaderMessage::new("block".to_string(), Some(&block_payload));
    let mut block_message = vec![];
    block_message.extend_from_slice(&header.to_le_bytes());
    block_message.extend_from_slice(&block_payload);
    block_message
}
