use super::message_header::{is_terminated, HeaderMessage};
use crate::blocks::block_header::BlockHeader;
use crate::compact_size_uint::CompactSizeUint;
use crate::logwriter::log_writer::{write_in_log, LogSender};
use std::fs::File;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, RwLock};
const BLOCK_HEADER_SIZE: usize = 80;
pub struct HeadersMessage;

impl HeadersMessage {
    /// Receives the response of the headers message in bytes.
    /// Returns a vector with the block headers contained.
    pub fn unmarshalling(
        headers_message_bytes: &Vec<u8>,
    ) -> Result<Vec<BlockHeader>, &'static str> {
        let mut block_header_vec: Vec<BlockHeader> = Vec::new();
        let mut offset: usize = 0;
        let count: CompactSizeUint =
            CompactSizeUint::unmarshalling(headers_message_bytes, &mut offset)?;
        let headers_size: usize = headers_message_bytes.len();
        let mut i: u64 = 0;
        while i < count.decoded_value() {
            if offset + BLOCK_HEADER_SIZE > headers_size {
                return Err("Out of bounds");
            }
            // The 1 is the transaction_count that comes as 0x00
            i += 1;
            block_header_vec.push(BlockHeader::unmarshalling(
                headers_message_bytes,
                &mut offset,
            )?);
            offset += 1;
        }

        Ok(block_header_vec)
    }
    /// Given a stream that implements the Read trait (from where you can read), reads the headers message and returns
    /// a vector with the headers if it could be read correctly or an Error otherwise.
    pub fn read_from(
        log_sender: &LogSender,
        stream: &mut TcpStream,
        finish: Option<Arc<RwLock<bool>>>,
    ) -> Result<Vec<BlockHeader>, Box<dyn std::error::Error>> {
        let header =
            HeaderMessage::read_from(log_sender, stream, "headers".to_string(), finish.clone())?;
        if is_terminated(finish) {
            let headers: Vec<BlockHeader> = Vec::new();
            return Ok(headers);
        }
        let payload_size = header.payload_size as usize;
        let mut buffer_num = vec![0; payload_size];
        stream.read_exact(&mut buffer_num)?;
        let mut vec: Vec<u8> = vec![];
        vec.extend_from_slice(&buffer_num);
        let headers = Self::unmarshalling(&vec)?;
        Ok(headers)
    }

    /// Stores the headers received in disk. Reads the headers from the stream and writes them in the file
    /// in the same format as they are read from the stream.
    pub fn read_from_node_and_write_to_file(
        log_sender: &LogSender,
        stream: &mut TcpStream,
        finish: Option<Arc<RwLock<bool>>>,
        file: &mut File,
    ) -> Result<Vec<BlockHeader>, Box<dyn std::error::Error>> {
        let header =
            HeaderMessage::read_from(log_sender, stream, "headers".to_string(), finish.clone())?;
        if is_terminated(finish) {
            let headers: Vec<BlockHeader> = Vec::new();
            return Ok(headers);
        }
        let payload_size = header.payload_size as usize;
        let mut buffer_num = vec![0; payload_size];
        stream.read_exact(&mut buffer_num)?;
        let mut vec: Vec<u8> = vec![];
        vec.extend_from_slice(&buffer_num);
        let headers = Self::unmarshalling(&vec)?;
        // write in file
        if let Err(err) = file.write_all(&vec) {
            write_in_log(
                &log_sender.error_log_sender,
                format!("Error trying to write in file: {:?}", err).as_str(),
            );
        }
        Ok(headers)
    }
    /// Given a vector of block headers, it builds the headers message and returns it in a vector of bytes.
    pub fn marshalling(headers: Vec<BlockHeader>) -> Vec<u8> {
        let mut headers_message_payload: Vec<u8> = Vec::new();
        let count = CompactSizeUint::new(headers.len() as u128);
        headers_message_payload.extend_from_slice(count.value());
        for header in headers {
            let mut header_bytes = vec![];
            header.marshalling(&mut header_bytes);
            header_bytes.extend_from_slice(&[0x00]); // transaction_count
            headers_message_payload.extend_from_slice(&header_bytes);
        }
        let header = HeaderMessage::new("headers".to_string(), Some(&headers_message_payload));
        let mut headers_message: Vec<u8> = Vec::new();
        headers_message.extend_from_slice(&header.to_le_bytes());
        headers_message.extend_from_slice(&headers_message_payload);
        headers_message
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        blocks::block_header::BlockHeader, compact_size_uint::CompactSizeUint,
        messages::headers_message::HeadersMessage,
    };

    #[test]
    fn test_deserialization_of_empty_headers_message_does_not_yield_block_headers(
    ) -> Result<(), &'static str> {
        let headers_message: Vec<u8> = vec![0; 1];
        let block_headers = HeadersMessage::unmarshalling(&headers_message)?;
        let expected_value = 0;
        assert_eq!(block_headers.len(), expected_value);
        Ok(())
    }

    #[test]
    fn test_deserialization_of_headers_message_returns_1_block_header() -> Result<(), &'static str>
    {
        let headers_message: Vec<u8> = vec![1; 82];
        let block_headers = HeadersMessage::unmarshalling(&headers_message)?;
        let expected_value = 1;
        assert_eq!(block_headers.len(), expected_value);
        Ok(())
    }

    #[test]
    fn test_deserialization_of_headers_message_returns_2_block_headers() -> Result<(), &'static str>
    {
        let headers_message: Vec<u8> = vec![2; 163];
        let block_headers = HeadersMessage::unmarshalling(&headers_message)?;
        let expected_value = 2;
        assert_eq!(block_headers.len(), expected_value);
        Ok(())
    }

    #[test]
    fn test_deserialization_of_headers_message_returns_correct_block_header(
    ) -> Result<(), &'static str> {
        let mut headers_message: Vec<u8> = vec![0; 82];
        for i in 1..83 {
            headers_message[i - 1] = i as u8;
        }

        let block_headers = HeadersMessage::unmarshalling(&headers_message)?;

        let mut expected_block_header_bytes: Vec<u8> = vec![2; 80];
        expected_block_header_bytes.copy_from_slice(&headers_message[1..81]);
        let mut offset: usize = 0;
        let expected_block_header =
            BlockHeader::unmarshalling(&expected_block_header_bytes, &mut offset)?;
        let received_block_header = &block_headers[0];

        assert_eq!(received_block_header.version, expected_block_header.version);
        assert_eq!(
            received_block_header.previous_block_header_hash,
            expected_block_header.previous_block_header_hash
        );
        assert_eq!(
            received_block_header.merkle_root_hash,
            expected_block_header.merkle_root_hash
        );
        assert_eq!(received_block_header.time, expected_block_header.time);
        assert_eq!(received_block_header.n_bits, expected_block_header.n_bits);
        assert_eq!(received_block_header.nonce, expected_block_header.nonce);
        assert_eq!(received_block_header.hash(), expected_block_header.hash());
        Ok(())
    }

    #[test]
    fn test_deserialization_of_headers_message_with_515_block_headers() -> Result<(), &'static str>
    {
        let mut headers_message: Vec<u8> = Vec::new();
        let count = CompactSizeUint::new(515);
        headers_message.extend_from_slice(count.value());

        for i in 0..(41718 - 3) {
            headers_message.push(i as u8);
        }
        let block_headers = HeadersMessage::unmarshalling(&headers_message)?;

        let mut expected_block_header_bytes: Vec<u8> = vec![2; 80];
        expected_block_header_bytes.copy_from_slice(&headers_message[3..83]);
        let mut offset: usize = 0;
        let expected_block_header =
            BlockHeader::unmarshalling(&expected_block_header_bytes, &mut offset)?;
        let received_block_header = &block_headers[0];
        let expected_len = 515;

        assert_eq!(block_headers.len(), expected_len);
        assert_eq!(received_block_header.version, expected_block_header.version);
        assert_eq!(
            received_block_header.previous_block_header_hash,
            expected_block_header.previous_block_header_hash
        );
        assert_eq!(
            received_block_header.merkle_root_hash,
            expected_block_header.merkle_root_hash
        );
        assert_eq!(received_block_header.time, expected_block_header.time);
        assert_eq!(received_block_header.n_bits, expected_block_header.n_bits);
        assert_eq!(received_block_header.nonce, expected_block_header.nonce);
        assert_eq!(received_block_header.hash(), expected_block_header.hash());
        Ok(())
    }
}
