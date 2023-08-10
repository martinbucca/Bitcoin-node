use super::message_header::*;
use super::payload::version_payload::{get_version_payload, VersionPayload};
use crate::config::Config;
use crate::logwriter::log_writer::LogSender;
use std::error::Error;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::net::TcpStream;
use std::str::Utf8Error;
use std::sync::Arc;

#[derive(Clone, Debug)]
/// Represents a "version" message according to the bitcoin protocol with a header (24 bytes) and a payload (variable)
pub struct VersionMessage {
    pub header: HeaderMessage,
    pub payload: VersionPayload,
}

impl VersionMessage {
    /// Receives a VersionMessage struct that represents a "version" message according to the bitcoin protocol
    /// and a stream that implements the Write trait (where you can write) and writes the serialized message
    /// in bytes in the stream. Returns an error if it could not be written correctly or an Ok if it was written correctly.
    pub fn write_to(&self, stream: &mut dyn Write) -> std::io::Result<()> {
        let header = self.header.to_le_bytes();
        let payload = self.payload.to_le_bytes();
        let mut message: Vec<u8> = Vec::new();
        message.extend_from_slice(&header);
        message.extend(payload);
        stream.write_all(&message)?;
        stream.flush()?;
        Ok(())
    }
    /// Receives a stream that implements the Read trait (where you can read) and reads the bytes that correspond to the
    /// version message according to the bitcoin protocol. Returns an error in case it cannot be read correctly 
    /// from the stream or in case the bytes read cannot be deserialized to a VersionMessage struct, otherwise,
    /// returns an Ok() with a VersionMessage deserialized from the bytes it read from the stream.
    pub fn read_from(
        log_sender: &LogSender,
        stream: &mut TcpStream,
    ) -> Result<VersionMessage, std::io::Error> {
        let header = HeaderMessage::read_from(log_sender, stream, "version".to_string(), None)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))?;
        let payload_large = header.payload_size;
        let mut buffer_num = vec![0; payload_large as usize];
        stream.read_exact(&mut buffer_num)?;
        let payload = VersionPayload::from_le_bytes(&buffer_num).map_err(|err: Utf8Error| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string())
        })?;
        Ok(VersionMessage { header, payload })
    }
}

/// Generates the VersionMessage with the received data and returns it.
/// In case of failure returns error.
pub fn get_version_message(
    config: &Arc<Config>,
    socket_addr: SocketAddr,
    local_ip_addr: SocketAddr,
) -> Result<VersionMessage, Box<dyn Error>> {
    let version_payload = get_version_payload(config, socket_addr, local_ip_addr)?;
    let version_header = HeaderMessage {
        start_string: config.start_string,
        command_name: "version".to_string(),
        payload_size: version_payload.to_le_bytes().len() as u32,
        checksum: get_checksum(&version_payload.to_le_bytes()),
    };
    Ok(VersionMessage {
        header: version_header,
        payload: version_payload,
    })
}
