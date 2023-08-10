use super::message_header::HeaderMessage;
use super::payload::getheaders_payload::GetHeadersPayload;
use crate::compact_size_uint::CompactSizeUint;
use crate::config::Config;
use std::error::Error;
use std::io::Write;
use std::sync::Arc;

/// Represents a message of the getheaders type according to the bitcoin protocol,
/// with its respective header and payload
pub struct GetHeadersMessage {
    pub header: HeaderMessage,
    pub payload: GetHeadersPayload,
}

impl GetHeadersMessage {
    /// Given a GetHeadersMessage struct and a stream that implements the Write trait where it can be written,
    /// writes the serialized message to bytes in the stream and returns an Ok() if it could be written correctly,
    /// otherwise an error if it was not written correctly in the stream
    pub fn write_to(&self, stream: &mut dyn Write) -> std::io::Result<()> {
        let header = self.header.to_le_bytes();
        let payload: Vec<u8> = self.payload.to_le_bytes();
        let mut message: Vec<u8> = Vec::new();
        message.extend_from_slice(&header);
        message.extend(payload);
        stream.write_all(&message)?;
        stream.flush()?;
        Ok(())
    }

    /// Given a vector of bytes, it tries to interpret the vec as a getheaders message.
    pub fn read_from(payload_bytes: &[u8]) -> Result<GetHeadersMessage, Box<dyn Error>> {
        let payload = GetHeadersPayload::read_from(payload_bytes)?;
        let header = HeaderMessage::new("getheaders".to_string(), Some(payload_bytes));
        Ok(GetHeadersMessage { header, payload })
    }
    /// Receives a Config struct with the constants to use in the header of the getheaders message and a vector
    /// of block hashes. Builds the getheaders message to request all the headers from the last hash in the vector
    /// of hashes and with stop_hash in 0 so that it returns 2000 or if it cannot return 2000, all it has.
    pub fn build_getheaders_message(
        config: &Arc<Config>,
        locator_hashes: Vec<[u8; 32]>,
    ) -> GetHeadersMessage {
        let hash_count = CompactSizeUint::new(1u128);
        let stop_hash = [0; 32];
        let getheaders_payload = GetHeadersPayload {
            version: config.protocol_version as u32,
            hash_count,
            locator_hashes,
            stop_hash,
        };
        let header_of_getheaders = HeaderMessage::new(
            "getheaders".to_string(),
            Some(&getheaders_payload.to_le_bytes()),
        );
        GetHeadersMessage {
            header: header_of_getheaders,
            payload: getheaders_payload,
        }
    }
}
