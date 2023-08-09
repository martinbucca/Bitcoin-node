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
/// Representa un mensaje "version" segun el protocolo de bitcoin con un header (24 bytes) y un payload (variable)
pub struct VersionMessage {
    pub header: HeaderMessage,
    pub payload: VersionPayload,
}

impl VersionMessage {
    /// Recibe un struct VersionMessage que representa un mensaje "version" segun protocolo de bitcoin
    /// y un stream que implemente el trait Write (en donde se pueda escribir) y escribe el mensaje serializado
    /// en bytes en el stream. Devuelve un error en caso de que no se haya podido escribir correctamente o un Ok en caso
    /// de que se haya escrito correctamente
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
    /// Recibe un stream que implementa el trait Read (de donde se puede leer) y lee los bytes que corresponden al
    /// mensaje version segun el protocolo de bitcoin. Devuelve error en caso de que se no se haya podido leer correctamente
    /// del stream o en caso de que los bytes leidos no puedan ser deserializados a un struct del VersionMessage, en caso
    /// contrario, devuelve un Ok() con un VersionMessage deserializado de los bytes que leyo del stream.
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

/// Genera el VersionMessage con los datos recibidos y lo devuelve
/// En caso que falle devuelve error
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
