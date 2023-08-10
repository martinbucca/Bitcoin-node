use crate::logwriter::log_writer::{write_in_log, LogSender};
use bitcoin_hashes::{sha256d, Hash};
use std::error::Error;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::str::Utf8Error;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use std::vec;

const START_STRING_TESTNET: [u8; 4] = [0x0b, 0x11, 0x09, 0x07];
const CHECKSUM_EMPTY_PAYLOAD: [u8; 4] = [0x5d, 0xf6, 0xe0, 0xe2];

#[derive(Clone, Debug)]
/// Represents the header of any message of the bitcoin protocol.
pub struct HeaderMessage {
    pub start_string: [u8; 4],
    pub command_name: String,
    pub payload_size: u32,
    pub checksum: [u8; 4],
}

impl HeaderMessage {
    /// Given the command name and an Option (if it is None represents that the command
    /// does not have payload or a Vec<u8> representing the payload of the message) 
    /// returns the HeaderMessage of that message.
    pub fn new(command_name: String, payload: Option<&[u8]>) -> Self {
        match payload {
            None => HeaderMessage {
                start_string: START_STRING_TESTNET,
                command_name,
                payload_size: 0,
                checksum: CHECKSUM_EMPTY_PAYLOAD,
            },
            Some(payload) => HeaderMessage {
                start_string: START_STRING_TESTNET,
                command_name,
                payload_size: payload.len() as u32,
                checksum: get_checksum(payload),
            },
        }
    }
    /// Converts the struct that represents the header of any message to bytes according 
    /// to the serialization rules of bitcoin protocol.
    pub fn to_le_bytes(&self) -> [u8; 24] {
        let mut header_message_bytes: [u8; 24] = [0; 24];
        header_message_bytes[0..4].copy_from_slice(&self.start_string);
        header_message_bytes[4..16].copy_from_slice(&command_name_to_bytes(&self.command_name));
        header_message_bytes[16..20].copy_from_slice(&self.payload_size.to_le_bytes());
        header_message_bytes[20..24].copy_from_slice(&self.checksum);
        header_message_bytes
    }
    /// Receives the bytes of a message header and converts them to a HeaderMessage struct
    /// according to the bitcoin protocol
    pub fn from_le_bytes(bytes: [u8; 24]) -> Result<Self, Utf8Error> {
        let mut start_string = [0; 4];
        let mut counter = 0;
        start_string[..4].copy_from_slice(&bytes[..4]);
        counter += 4;
        let mut command_name_bytes = [0; 12];
        command_name_bytes[..12].copy_from_slice(&bytes[counter..(12 + counter)]);
        counter += 12;
        let command_name = std::str::from_utf8(&command_name_bytes)?.to_string();
        let mut payload_size_bytes: [u8; 4] = [0; 4];
        payload_size_bytes[..4].copy_from_slice(&bytes[counter..(4 + counter)]);
        counter += 4;
        let payload_size = u32::from_le_bytes(payload_size_bytes);
        let mut checksum = [0; 4];
        checksum[..4].copy_from_slice(&bytes[counter..(4 + counter)]);
        Ok(HeaderMessage {
            start_string,
            command_name,
            payload_size,
            checksum,
        })
    }
    /// Receives a HeaderMessage struct that represents a message header according to the bitcoin protocol
    /// and a stream that implements the Write trait (where you can write) and writes the serialized message
    /// in bytes in the stream. Returns an error if it could not be written correctly or an Ok(()) if it was written correctly.
    pub fn write_to(&self, stream: &mut dyn Write) -> Result<(), Box<dyn std::error::Error>> {
        let header = self.to_le_bytes();
        stream.write_all(&header)?;
        stream.flush()?;
        Ok(())
    }
    /// Recibe un stream que implemente el trait read (algo desde lo que se pueda leer) y el nombre del comando que se quiere leer
    /// y devuelve un HeaderMessage si se pudo leer correctamente uno desde el stream
    /// o Error si lo leido no corresponde a el header de un mensaje del protocolo de bitcoin
    /// Receives a stream that implements the Read trait (something from which you can read) and the name of the command you want to read
    /// and returns a HeaderMessage if it could be read correctly from the stream, error otherwise.
    pub fn read_from(
        log_sender: &LogSender,
        mut stream: &mut TcpStream,
        command_name: String,
        finish: Option<Arc<RwLock<bool>>>,
    ) -> Result<Self, Box<dyn Error>> {
        if command_name == *"block" {
            // will wait a minimum of two more seconds for the stalling node to send the block.
            // If the block still hasn’t arrived, Bitcoin Core will disconnect from the stalling
            // node and attempt to connect to another node.
            stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        }
        let header_command_name =
            std::str::from_utf8(&command_name_to_bytes(&command_name))?.to_string();
        let mut buffer_num = [0; 24];
        stream.read_exact(&mut buffer_num)?;
        let mut header = HeaderMessage::from_le_bytes(buffer_num)?;
        // if the wanted header was not read, keep reading until it is find or the program is terminated
        while header.command_name != header_command_name && !is_terminated(finish.clone()) {
            let payload = read_payload(&mut stream, &header)?;
            if header.command_name.contains("ping") {
                write_in_log(
                    &log_sender.message_log_sender,
                    format!(
                        "Message received correctly: ping -- Node: {:?}",
                        stream.peer_addr()?
                    )
                    .as_str(),
                );
                write_pong_message(&mut stream, &payload)?;
            }
            write_in_log(
                &log_sender.message_log_sender,
                format!(
                    "IGNORED -- Message received: {} -- Node: {:?}",
                    header.command_name,
                    stream.peer_addr()?
                )
                .as_str(),
            );

            buffer_num = [0; 24];
            stream.read_exact(&mut buffer_num)?;
            header = HeaderMessage::from_le_bytes(buffer_num)?;
        }
        if !is_terminated(finish) {
            write_in_log(
                &log_sender.message_log_sender,
                format!(
                    "Message received correctly: {} -- Node: {:?}",
                    command_name,
                    stream.peer_addr()?
                )
                .as_str(),
            );
        }
        Ok(header)
    }
}

/// Checks the received finish variable.
/// Returns true or false depending on whether the program should end.
pub fn is_terminated(finish: Option<Arc<RwLock<bool>>>) -> bool {
    match finish {
        Some(m) => *m.read().unwrap(),
        None => false,
    }
}

/// Receives the HeaderMessage and reads the corresponding payload from the stream.
/// Returns the bytes read from the stream
fn read_payload(stream: &mut dyn Read, header: &HeaderMessage) -> io::Result<Vec<u8>> {
    let payload_size = header.payload_size as usize;
    let mut payload_buffer_num: Vec<u8> = vec![0; payload_size];
    stream.read_exact(&mut payload_buffer_num)?;
    Ok(payload_buffer_num)
}

/// Recibe un stream que implemente el trait Write (algo donde se pueda escribir) y escribe el mensaje verack segun
/// el protocolo de bitcoin, si se escribe correctamente devuelve Ok(()) y sino devuelve un error
/// Receives a stream that implements the Write trait (something where you can write) and writes the verack message according to
/// the bitcoin protocol. If it is written correctly it returns Ok(()), otherwise it returns an error.
pub fn write_verack_message(stream: &mut dyn Write) -> Result<(), Box<dyn std::error::Error>> {
    let header = HeaderMessage::new("verack".to_string(), None);
    header.write_to(stream)?;
    Ok(())
}

/// Receives a stream that implements the Write trait (something where you can write) and 
/// the nonce of the ping message to which it must respond and writes the pong message 
/// according to the bitcoin protocol. If it is written 
/// correctly it returns Ok(()), otherwise it returns an error.
pub fn write_pong_message(
    stream: &mut dyn Write,
    payload: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let header = HeaderMessage::new("pong".to_string(), Some(payload));
    let header_bytes = HeaderMessage::to_le_bytes(&header);
    let mut message: Vec<u8> = Vec::new();
    message.extend_from_slice(&header_bytes);
    message.extend(payload);
    stream.write_all(&message)?;
    stream.flush()?;
    Ok(())
}

/// Receives a stream that implements the Write trait (something where you can write) and 
/// writes the sendheaders message according to the bitcoin protocol. If it is written
/// correctly it returns Ok(()), otherwise it returns an error.
pub fn write_sendheaders_message(stream: &mut dyn Write) -> Result<(), Box<dyn std::error::Error>> {
    let header = HeaderMessage::new("sendheaders".to_string(), None);
    header.write_to(stream)?;
    Ok(())
}
/// Receives a stream that implements the Read trait (something from which you can read) and 
/// reads the verack message according to the bitcoin protocol. If it is read correctly it returns
/// Ok(HeaderMessage), otherwise it returns an error.
pub fn read_verack_message(
    log_sender: &LogSender,
    stream: &mut TcpStream,
) -> Result<HeaderMessage, Box<dyn std::error::Error>> {
    HeaderMessage::read_from(log_sender, stream, "verack".to_string(), None)
}

/// Receives a String that represents the name of the Header Message command
/// and returns the bytes that represent that string (ASCII) followed by 0x00 to
/// complete the 12 bytes
/// little-endian
pub fn command_name_to_bytes(command: &String) -> [u8; 12] {
    let mut command_name_bytes = [0; 12];
    let command_bytes = command.as_bytes();
    command_name_bytes[..command_bytes.len()]
        .copy_from_slice(&command_bytes[..command_bytes.len()]);
    command_name_bytes
}

/// Generates the checksum of the received payload.
/// Returns the 4 bytes of the checksum.
pub fn get_checksum(payload: &[u8]) -> [u8; 4] {
    let sha_hash = sha256d::Hash::hash(payload); // double sha256 of payload
    let hash_bytes: [u8; 32] = sha_hash.to_byte_array(); // convert Hash to [u8; 32] array
    let mut checksum: [u8; 4] = [0u8; 4];
    checksum.copy_from_slice(&hash_bytes[0..4]); // checksum returns the first 4 bytes of SHA256(SHA256(payload))
    checksum
}
#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn header_message_bytes_from_verack_message_unmarshalling_correctly(
    ) -> Result<(), Box<dyn Error>> {
        // GIVEN: a header message of verack message in bytes
        let header_message_bytes: [u8; 24] = [
            11, 17, 9, 7, 118, 101, 114, 97, 99, 107, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 93, 246, 224,
            226,
        ];
        // WHEN: the from_le_bytes function of the HeaderMessage struct is executed with the provided bytes
        let header = HeaderMessage::from_le_bytes(header_message_bytes)?;
        // THEN: a HeaderMessage struct is returned with correct fields according to verack message
        assert_eq!([11u8, 17u8, 9u8, 7u8], header.start_string);
        assert_eq!("verack\0\0\0\0\0\0", header.command_name);
        assert_eq!(0, header.payload_size);
        assert_eq!([93u8, 246u8, 224u8, 226u8], header.checksum);
        Ok(())
    }

    #[test]
    fn header_message_bytes_from_version_message_unmarshalling_correctly(
    ) -> Result<(), Box<dyn Error>> {
        // GIVEN: a header message of version message in bytes
        let header_message_bytes: [u8; 24] = [
            11, 17, 9, 7, 118, 101, 114, 115, 105, 111, 110, 0, 0, 0, 0, 0, 100, 0, 0, 0, 152, 16,
            0, 0,
        ];
        // WHEN: the from_le_bytes function of the HeaderMessage struct is executed with the provided bytes
        let header = HeaderMessage::from_le_bytes(header_message_bytes)?;
        // THEN: a HeaderMessage struct is returned with correct fields according to version message
        assert_eq!([11u8, 17u8, 9u8, 7u8], header.start_string);
        assert_eq!("version\0\0\0\0\0", header.command_name);
        assert_eq!(100, header.payload_size);
        assert_eq!([152u8, 16u8, 0u8, 0u8], header.checksum);
        Ok(())
    }

    #[test]
    fn error_when_command_name_bytes_cannot_be_represented_as_string() {
        // GIVEN: a header message of a message with erroneous command name in bytes
        let header_message_bytes: [u8; 24] = [
            11, 17, 9, 7, 12, 101, 114, 13, 240, 111, 110, 1, 0, 0, 0, 11, 100, 0, 0, 0, 152, 16,
            0, 0,
        ];
        // WHEN: the from_le_bytes function of the HeaderMessage struct is executed with the provided bytes
        let header = HeaderMessage::from_le_bytes(header_message_bytes);
        // THEN: header is an error
        assert!(header.is_err());
        assert!(matches!(header, Err(_)));
    }

    #[test]
    fn header_message_of_verack_message_marshalling_correctly_to_bytes() {
        // GIVEN: a HeaderMessage struct of a verack message
        let verack_header_message = HeaderMessage {
            start_string: [11, 17, 9, 7],
            command_name: "verack".to_string(),
            payload_size: 0,
            checksum: [93, 246, 224, 226],
        };
        // WHEN: the to_le_bytes function is executed on the HeaderMessage struct
        let header_message_bytes = verack_header_message.to_le_bytes();
        // THEN: it converts to correct bytes according to verack message
        let expected_bytes_from_verack_header_message: [u8; 24] = [
            11, 17, 9, 7, 118, 101, 114, 97, 99, 107, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 93, 246, 224,
            226,
        ];
        assert_eq!(
            expected_bytes_from_verack_header_message,
            header_message_bytes
        );
    }

    #[test]
    fn header_message_of_version_message_marshalling_correctly_to_bytes() {
        // GIVEN: a HeaderMessage struct of a version message
        let version_header_message = HeaderMessage {
            start_string: [11, 17, 9, 7],
            command_name: "version".to_string(),
            payload_size: 100,
            checksum: [152, 16, 0, 0],
        };
        // WHEN: the to_le_bytes function is executed on the HeaderMessage struct
        let header_message_bytes = version_header_message.to_le_bytes();
        // THEN: it converts to correct bytes according to version message
        let expected_bytes_from_version_header_message: [u8; 24] = [
            11, 17, 9, 7, 118, 101, 114, 115, 105, 111, 110, 0, 0, 0, 0, 0, 100, 0, 0, 0, 152, 16,
            0, 0,
        ];
        assert_eq!(
            expected_bytes_from_version_header_message,
            header_message_bytes
        );
    }
}
