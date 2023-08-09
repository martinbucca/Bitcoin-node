use super::message_header::HeaderMessage;
use super::payload::getheaders_payload::GetHeadersPayload;
use crate::compact_size_uint::CompactSizeUint;
use crate::config::Config;
use std::error::Error;
use std::io::Write;
use std::sync::Arc;

/// Representa un mensaje del tipo getheaders segun el protocolo de bitcoin, con su respectivo header y payload
pub struct GetHeadersMessage {
    pub header: HeaderMessage,
    pub payload: GetHeadersPayload,
}

impl GetHeadersMessage {
    /// Dado un struct GetHeadersMessage y un stream que implemente el trait Write en donde se pueda escribir,
    /// escribe el mensaje serializado a bytes en el stream y devuelve un Ok() si lo pudo escribir correctamente,
    /// y un error si no se escribio correctamente en el stream
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

    /// Dado un vector de bytes, intenta interpretar el mismo como un mensaje getheaders
    pub fn read_from(payload_bytes: &[u8]) -> Result<GetHeadersMessage, Box<dyn Error>> {
        let payload = GetHeadersPayload::read_from(payload_bytes)?;
        let header = HeaderMessage::new("getheaders".to_string(), Some(payload_bytes));
        Ok(GetHeadersMessage { header, payload })
    }
    /// Recibe un struct Config con las constantes a utilizar en el header del mensaje getheaders y un vector
    /// de hashes de bloques y arma el mensaje getheaders para que pida todos los headers a partir del ultimo hash del vector
    /// de hashes y con stop_hash en 0 para que devuelva 2000 o si no puede devolver 2000, todos los que tenga
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
