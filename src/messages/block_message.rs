use std::{error::Error, io::Read, net::TcpStream};

use crate::{blocks::block::Block, logwriter::log_writer::LogSender};

use super::message_header::HeaderMessage;

#[derive(Debug)]
/// Represents the block message that is received in response to the getdata message.
pub struct BlockMessage;

impl BlockMessage {
    /// Receives the "block" message in bytes.
    /// Returns a block
    pub fn unmarshalling(block_message_payload_bytes: &Vec<u8>) -> Result<Block, Box<dyn Error>> {
        let mut offset = 0;
        let block = Block::unmarshalling(block_message_payload_bytes, &mut offset)?;
        Ok(block)
    }
    /// Given a stream that implements the Read trait (from where it can be read) it reads the block message and returns
    /// the corresponding block if it could be read correctly or an Error otherwise.
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

/// Returns the block message with the block passed by parameter.
pub fn get_block_message(block: &Block) -> Vec<u8> {
    let mut block_payload = vec![];
    block.marshalling(&mut block_payload);
    let header = HeaderMessage::new("block".to_string(), Some(&block_payload));
    let mut block_message = vec![];
    block_message.extend_from_slice(&header.to_le_bytes());
    block_message.extend_from_slice(&block_payload);
    block_message
}
