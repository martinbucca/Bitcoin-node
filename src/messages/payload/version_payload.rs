use crate::compact_size_uint::CompactSizeUint;
use crate::config::Config;
use rand::Rng;
use std::error::Error;
use std::net::SocketAddr;
use std::str::Utf8Error;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
///  Representa el payload de un mensaje Version segun el protocolo bitcoin, con todos sus respectivos campos
/// (corresponde a la version del protocolo 70015)
pub struct VersionPayload {
    pub version: i32,                      // highest protocol version.
    pub services: u64,                     // services supported by our node.
    pub timestamp: i64, // The current Unix epoch time according to the transmitting node’s clock.
    pub addr_recv_service: u64, // The services supported by the receiving node as perceived by the transmitting node.
    pub addr_recv_ip: [u8; 16], // The IPv6 address of the receiving node as perceived by the transmitting node in big endian byte order.
    pub addr_recv_port: u16, // The port number of the receiving node as perceived by the transmitting node in big endian byte order.
    pub addr_trans_service: u64, // The services supported by the transmitting node.
    pub addr_trans_ip: [u8; 16], // The IPv6 address of the transmitting node in big endian byte order.
    pub addr_trans_port: u16, // The port number of the transmitting node in big endian byte order.
    pub nonce: u64,           // A random nonce which can help a node detect a connection to itself.
    pub user_agent_bytes: CompactSizeUint, // Number of bytes in following user_agent field.
    pub user_agent: String,   // User agent as defined by BIP14.
    pub start_height: i32, // The height of the transmitting node’s best block chain or, in the case of an SPV client, best block header chain.
    pub relay: bool,       // Transaction relay flag.
}

/// Recibe un vector de bytes y un contador que representa las posiciones leidas del vector y devuelve
/// un i32 deserializado de los bytes, que representa el campo "version" del payload del mensaje version e incrementa el contador en la cantidad
/// de bytes leidos (4)
fn get_version_from_bytes(bytes: &[u8], counter: &mut usize) -> i32 {
    let mut version_bytes = [0; 4];
    version_bytes[..4].copy_from_slice(&bytes[..4]);
    let version = i32::from_le_bytes(version_bytes);
    *counter += 4;
    version
}
/// Recibe un vector de bytes y un contador que representa las posiciones leidas del vector y devuelve
/// un u64 deserializado de los bytes, que representa el campo "services" del payload del mensaje version e incrementa el contador en la cantidad
/// de bytes leidos (8)
fn get_services_from_bytes(bytes: &[u8], counter: &mut usize) -> u64 {
    let mut services_bytes: [u8; 8] = [0; 8];
    services_bytes[..8].copy_from_slice(&bytes[*counter..(8 + *counter)]);
    let services = u64::from_le_bytes(services_bytes);
    *counter += 8;
    services
}
/// Recibe un vector de bytes y un contador que representa las posiciones leidas del vector y devuelve
/// un i64 deserializado de los bytes, que representa el campo "timestamp" del payload del mensaje version e incrementa el contador en la cantidad
/// de bytes leidos (8)
fn get_timestamp_from_bytes(bytes: &[u8], counter: &mut usize) -> i64 {
    let mut timestamp_bytes: [u8; 8] = [0; 8];
    timestamp_bytes[..8].copy_from_slice(&bytes[*counter..(8 + *counter)]);
    let timestamp = i64::from_le_bytes(timestamp_bytes);
    *counter += 8;
    timestamp
}
/// Recibe un vector de bytes y un contador que representa las posiciones leidas del vector y devuelve
/// un u64 deserializado de los bytes, que representa el campo "addr_services" (tanto para recv como para trans nodes) del payload del mensaje version e incrementa el contador en la cantidad
/// de bytes leidos (8)
fn get_addr_services_from_bytes(bytes: &[u8], counter: &mut usize) -> u64 {
    let mut addr_recv_services_bytes: [u8; 8] = [0; 8];
    addr_recv_services_bytes[..8].copy_from_slice(&bytes[*counter..(8 + *counter)]);
    let addr_recv_service = u64::from_le_bytes(addr_recv_services_bytes);
    *counter += 8;
    addr_recv_service
}
/// Recibe un vector de bytes y un contador que representa las posiciones leidas del vector y devuelve
/// un vector de 16 bytes , que representa el campo "addr_ip" (tanto para recv como para trans nodes) del payload del mensaje version e incrementa el contador en la cantidad
/// de bytes leidos (16)
fn get_addr_ip_from_bytes(bytes: &[u8], counter: &mut usize) -> [u8; 16] {
    let mut addr_recv_ip: [u8; 16] = [0; 16];
    addr_recv_ip[..16].copy_from_slice(&bytes[*counter..(16 + *counter)]); // ya deberian estar en big endian
    *counter += 16;
    addr_recv_ip
}
/// Recibe un vector de bytes y un contador que representa las posiciones leidas del vector y devuelve
/// un u16 deserializado de los bytes, que representa el campo "addr_port" (tanto para recv como para trans nodes) del payload del mensaje version e incrementa el contador en la cantidad
/// de bytes leidos (2)
fn get_addr_port_from_bytes(bytes: &[u8], counter: &mut usize) -> u16 {
    let mut addr_recv_port_bytes: [u8; 2] = [0; 2];
    addr_recv_port_bytes[..2].copy_from_slice(&bytes[*counter..(2 + *counter)]);
    let addr_recv_port = u16::from_be_bytes(addr_recv_port_bytes);
    *counter += 2;
    addr_recv_port
}
/// Recibe un vector de bytes y un contador que representa las posiciones leidas del vector y devuelve
/// un u64 deserializado de los bytes, que representa el campo "nonce" del payload del mensaje version e incrementa el contador en la cantidad
/// de bytes leidos (8)
fn get_nonce_from_bytes(bytes: &[u8], counter: &mut usize) -> u64 {
    let mut nonce_bytes: [u8; 8] = [0; 8];
    nonce_bytes[..8].copy_from_slice(&bytes[*counter..(8 + *counter)]);
    let nonce = u64::from_le_bytes(nonce_bytes);
    *counter += 8;
    nonce
}
/// Recibe un vector de bytes y un contador que representa las posiciones leidas del vector y devuelve
/// un CompactSizeUint deserializado de los bytes, que representa el campo "user_agent_bytes" del payload del mensaje version e incrementa el contador en la cantidad
/// de bytes leidos (variable). si hay error en el unmarshalling devuelve un CompactSizeUnit en 0
fn get_user_agent_bytes_from_bytes(bytes: &[u8], counter: &mut usize) -> CompactSizeUint {
    let user_agent_bytes = CompactSizeUint::unmarshalling(bytes, &mut *counter);
    match user_agent_bytes {
        Ok(valor) => valor,
        Err(_e) => CompactSizeUint::new(0),
    }
}
/// Recibe un vector de bytes y un contador que representa las posiciones leidas del vector y devuelve
/// un i32 deserializado de los bytes, que representa el campo "start_height" del payload del mensaje version e incrementa el contador en la cantidad
/// de bytes leidos (4)
fn get_start_height_from_bytes(bytes: &[u8], counter: &mut usize) -> i32 {
    let mut start_height_bytes: [u8; 4] = [0; 4];
    start_height_bytes[..4].copy_from_slice(&bytes[*counter..(4 + *counter)]);
    let start_height = i32::from_le_bytes(start_height_bytes);
    *counter += 4;
    start_height
}
/// Recibe un vector de bytes y un contador que representa las posiciones leidas del vector y devuelve
/// un bool deserializado del byte leido, que representa el campo "relay" del payload del mensaje version
fn get_relay_from_bytes(bytes: &[u8], counter: usize) -> bool {
    let relay_byte = bytes[counter];
    matches!(relay_byte, 1u8)
}
/// Recibe un vector de bytes, un contador que representa las posiciones leidas del vector y la cantidad de bytes a leer del vector y devuelve
/// un String deserializado de los bytes leidos, que representa el campo "user_agent" del payload del mensaje version,  en caso de poder ser transformado
/// de bytes a string o devuelve un error en caso contrario
fn get_user_agent_from_bytes(
    bytes: &[u8],
    counter: &mut usize,
    user_agent_bytes: u64,
) -> Result<String, Utf8Error> {
    let mut user_agent_b = vec![0; user_agent_bytes as usize];
    user_agent_b.copy_from_slice(&bytes[*counter..(user_agent_bytes as usize + *counter)]);
    let user_agent = std::str::from_utf8(&user_agent_b)?.to_string();
    *counter += user_agent_bytes as usize;
    Ok(user_agent)
}

impl VersionPayload {
    /// Convierte el struct que representa el payload del  mensaje "version" a bytes segun las reglas de
    /// serializacion del protocolo bitcoin
    pub fn to_le_bytes(&self) -> Vec<u8> {
        let mut version_payload_bytes: Vec<u8> = vec![];
        version_payload_bytes.extend_from_slice(&self.version.to_le_bytes());
        version_payload_bytes.extend_from_slice(&self.services.to_le_bytes());
        version_payload_bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        version_payload_bytes.extend_from_slice(&self.addr_recv_service.to_le_bytes());
        version_payload_bytes.extend_from_slice(&self.addr_recv_ip); // big endian bytes
        version_payload_bytes.extend_from_slice(&self.addr_recv_port.to_be_bytes()); // big endian bytes
        version_payload_bytes.extend_from_slice(&self.addr_trans_service.to_le_bytes());
        version_payload_bytes.extend_from_slice(&self.addr_trans_ip); // big endian bytes
        version_payload_bytes.extend_from_slice(&self.addr_trans_port.to_be_bytes()); // big endian bytes
        version_payload_bytes.extend_from_slice(&self.nonce.to_le_bytes());
        version_payload_bytes.extend_from_slice(&self.user_agent_bytes.marshalling());
        version_payload_bytes.extend_from_slice(self.user_agent.as_bytes()); // little -> depende arq de computadora ??
        version_payload_bytes.extend_from_slice(&self.start_height.to_le_bytes());
        version_payload_bytes.push(self.relay as u8);
        version_payload_bytes
    }
    /// recibe los bytes de un payload de un mensaje "version" y los convierte a un struct VersionPayload
    /// de acuerdo al protocolo de bitcoin. Devuelve error en caso que no se puedan transformar los bytes correspondiente
    /// al campo user_agent a string.
    pub fn from_le_bytes(bytes: &[u8]) -> Result<Self, Utf8Error> {
        let mut counter = 0;
        let version = get_version_from_bytes(bytes, &mut counter);
        let services = get_services_from_bytes(bytes, &mut counter);
        let timestamp = get_timestamp_from_bytes(bytes, &mut counter);
        let addr_recv_service = get_addr_services_from_bytes(bytes, &mut counter);
        let addr_recv_ip = get_addr_ip_from_bytes(bytes, &mut counter);
        let addr_recv_port = get_addr_port_from_bytes(bytes, &mut counter);
        let addr_trans_service = get_addr_services_from_bytes(bytes, &mut counter);
        let addr_trans_ip = get_addr_ip_from_bytes(bytes, &mut counter);
        let addr_trans_port = get_addr_port_from_bytes(bytes, &mut counter);
        let nonce = get_nonce_from_bytes(bytes, &mut counter);
        let user_agent_bytes = get_user_agent_bytes_from_bytes(bytes, &mut counter);
        let user_agent =
            get_user_agent_from_bytes(bytes, &mut counter, user_agent_bytes.decoded_value())?;
        let start_height = get_start_height_from_bytes(bytes, &mut counter);
        let relay = get_relay_from_bytes(bytes, counter);
        Ok(VersionPayload {
            version,
            services,
            timestamp,
            addr_recv_service,
            addr_recv_ip,
            addr_recv_port,
            addr_trans_service,
            addr_trans_ip,
            addr_trans_port,
            nonce,
            user_agent_bytes,
            user_agent,
            start_height,
            relay,
        })
    }
}

/// Devuelve el tiempo acutal segun EPOCH como un i64 o error en caso de que no se pueda obtener
pub fn get_current_unix_epoch_time() -> Result<i64, Box<dyn Error>> {
    let current_time = SystemTime::now();
    let unix_epoch = UNIX_EPOCH;
    let unix_time = current_time.duration_since(unix_epoch)?;
    let seconds = unix_time.as_secs() as i64;
    Ok(seconds)
}
/// Recibe un address de un socket y devuelve un vector [u8; 16] que representa la direccion del socket
pub fn get_ipv6_address_ip(socket_addr: SocketAddr) -> [u8; 16] {
    let mut addr_recv_ip: [u8; 16] = [0; 16];
    let addr_recv_ip_aux: [u16; 8] = match socket_addr {
        SocketAddr::V4(addr) => addr.ip().to_ipv6_mapped().segments(),
        SocketAddr::V6(addr) => addr.ip().segments(),
    };
    for (i, num) in addr_recv_ip_aux.iter().enumerate() {
        let bytes = num.to_be_bytes(); // convertimos a bytes en orden de bytes de menor a mayor
        addr_recv_ip[(i * 2)..(i * 2 + 2)].copy_from_slice(&bytes); // copiamos los bytes en el vector de 8 bits
    }
    addr_recv_ip
}

/// Genera el payload para el mensaje version del protocolo bitcoin.
pub fn get_version_payload(
    config: &Arc<Config>,
    socket_addr: SocketAddr,
    local_ip_addr: SocketAddr,
) -> Result<VersionPayload, Box<dyn Error>> {
    let timestamp: i64 = get_current_unix_epoch_time()?;
    Ok(VersionPayload {
        version: config.protocol_version,
        services: 0u64,
        timestamp,
        addr_recv_service: 1u64,
        addr_recv_ip: get_ipv6_address_ip(socket_addr),
        addr_recv_port: 18333,
        addr_trans_service: 0u64,
        addr_trans_ip: get_ipv6_address_ip(local_ip_addr),
        addr_trans_port: 18333,
        nonce: rand::thread_rng().gen(),
        user_agent_bytes: CompactSizeUint::new(16u128),
        user_agent: config.user_agent.to_string(),
        start_height: 1,
        relay: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn get_version_from_payload_bytes_returns_the_correct_i32() {
        // GIVEN: Payload bytes de un mensaje version
        let payload_bytes: [u8; 102] = [
            127, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 253, 244, 83, 100, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 54, 89, 113, 236, 71, 157, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 192, 168, 0, 58, 71, 157, 51, 165, 53,
            24, 235, 29, 226, 36, 15, 47, 83, 97, 116, 111, 115, 104, 105, 58, 50, 51, 46, 48, 46,
            48, 47, 1, 0, 0, 0, 1,
        ];
        // WHEN: se ejecuta la funcion get_version_from_bytes con los bytes pasados por parametro
        let version = get_version_from_bytes(&payload_bytes, &mut 0);
        // THEN: el numero de version es el correcto
        assert_eq!(70015 as i32, version);
    }
    #[test]
    fn get_services_from_payload_bytes_returns_the_correct_u64() {
        // GIVEN: Payload bytes de un mensaje version
        let payload_bytes: [u8; 102] = [
            127, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 253, 244, 83, 100, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 54, 89, 113, 236, 71, 157, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 192, 168, 0, 58, 71, 157, 51, 165, 53,
            24, 235, 29, 226, 36, 15, 47, 83, 97, 116, 111, 115, 104, 105, 58, 50, 51, 46, 48, 46,
            48, 47, 1, 0, 0, 0, 1,
        ];
        // WHEN: se ejecuta la funcion get_services_from_bytes con los bytes pasados por parametro
        let services = get_services_from_bytes(&payload_bytes, &mut 4);
        // THEN: el numero de services es el correcto
        assert_eq!(0 as u64, services);
    }
    #[test]
    fn get_timestamp_from_payload_bytes_returns_the_correct_i64() {
        // GIVEN: Payload bytes de un mensaje version
        let payload_bytes: [u8; 102] = [
            127, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 253, 244, 83, 100, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 54, 89, 113, 236, 71, 157, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 192, 168, 0, 58, 71, 157, 51, 165, 53,
            24, 235, 29, 226, 36, 15, 47, 83, 97, 116, 111, 115, 104, 105, 58, 50, 51, 46, 48, 46,
            48, 47, 1, 0, 0, 0, 1,
        ];
        // WHEN: se ejecuta la funcion get_timestamp_from_bytes con los bytes pasados por parametro
        let timestamp = get_timestamp_from_bytes(&payload_bytes, &mut 12);
        let mut timestamp_bytes: [u8; 8] = [0; 8];
        timestamp_bytes[..8].copy_from_slice(&payload_bytes[12..20]);
        // THEN: el numero del timestamp es el correcto
        assert_eq!(i64::from_le_bytes(timestamp_bytes), timestamp);
    }
    #[test]
    fn get_addr_recv_service_from_payload_bytes_returns_the_correct_u64() {
        // GIVEN: Payload bytes de un mensaje version
        let payload_bytes: [u8; 102] = [
            127, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 253, 244, 83, 100, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 54, 89, 113, 236, 71, 157, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 192, 168, 0, 58, 71, 157, 51, 165, 53,
            24, 235, 29, 226, 36, 15, 47, 83, 97, 116, 111, 115, 104, 105, 58, 50, 51, 46, 48, 46,
            48, 47, 1, 0, 0, 0, 1,
        ];
        // WHEN: se ejecuta la funcion get_addr_services_from_bytes con los bytes pasados por parametro
        let addr_recv_service = get_addr_services_from_bytes(&payload_bytes, &mut 20);
        // THEN: el numero de addr_recv_services es el correcto
        assert_eq!(1u64, addr_recv_service);
    }
    #[test]
    fn get_addr_recv_ip_from_payload_bytes_returns_the_correct_16_bytes_of_ip_direction() {
        // GIVEN: Payload bytes de un mensaje version
        let payload_bytes: [u8; 102] = [
            127, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 253, 244, 83, 100, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 54, 89, 113, 236, 71, 157, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 192, 168, 0, 58, 71, 157, 51, 165, 53,
            24, 235, 29, 226, 36, 15, 47, 83, 97, 116, 111, 115, 104, 105, 58, 50, 51, 46, 48, 46,
            48, 47, 1, 0, 0, 0, 1,
        ];
        // WHEN: se ejecuta la funcion get_addr_ip_from_bytes con los bytes pasados por parametro
        let addr_recv_ip = get_addr_ip_from_bytes(&payload_bytes, &mut 28);
        let mut addr_recv_ip_bytes: [u8; 16] = [0; 16];
        addr_recv_ip_bytes[..16].copy_from_slice(&payload_bytes[28..44]);
        // THEN: el vector de addr_recv_ip es el correcto
        assert_eq!(addr_recv_ip_bytes, addr_recv_ip);
    }
    #[test]
    fn get_addr_recv_port_from_payload_bytes_returns_the_correct_u16() {
        // GIVEN: Payload bytes de un mensaje version
        let payload_bytes: [u8; 102] = [
            127, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 253, 244, 83, 100, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 54, 89, 113, 236, 71, 157, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 192, 168, 0, 58, 71, 157, 51, 165, 53,
            24, 235, 29, 226, 36, 15, 47, 83, 97, 116, 111, 115, 104, 105, 58, 50, 51, 46, 48, 46,
            48, 47, 1, 0, 0, 0, 1,
        ];
        // WHEN: se ejecuta la funcion get_addr_port_from_bytes con los bytes pasados por parametro
        let addr_recv_port = get_addr_port_from_bytes(&payload_bytes, &mut 44);
        // THEN: el numero de addr_recv_port es el correcto
        assert_eq!(18333u16, addr_recv_port);
    }
    #[test]
    fn get_addr_trans_service_from_payload_bytes_returns_the_correct_u64() {
        // GIVEN: Payload bytes de un mensaje version
        let payload_bytes: [u8; 102] = [
            127, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 253, 244, 83, 100, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 54, 89, 113, 236, 71, 157, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 192, 168, 0, 58, 71, 157, 51, 165, 53,
            24, 235, 29, 226, 36, 15, 47, 83, 97, 116, 111, 115, 104, 105, 58, 50, 51, 46, 48, 46,
            48, 47, 1, 0, 0, 0, 1,
        ];
        // WHEN: se ejecuta la funcion get_addr_services_from_bytes con los bytes pasados por parametro
        let addr_trans_service = get_addr_services_from_bytes(&payload_bytes, &mut 46);
        // THEN: el numero de addr_trans_services es el correcto
        assert_eq!(0u64, addr_trans_service);
    }
    #[test]
    fn get_addr_trans_ip_from_payload_bytes_returns_the_correct_16_bytes_of_ip_direction() {
        // GIVEN: Payload bytes de un mensaje version
        let payload_bytes: [u8; 102] = [
            127, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 253, 244, 83, 100, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 54, 89, 113, 236, 71, 157, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 192, 168, 0, 58, 71, 157, 51, 165, 53,
            24, 235, 29, 226, 36, 15, 47, 83, 97, 116, 111, 115, 104, 105, 58, 50, 51, 46, 48, 46,
            48, 47, 1, 0, 0, 0, 1,
        ];
        // WHEN: se ejecuta la funcion get_addr_ip_from_bytes con los bytes pasados por parametro
        let addr_trans_ip = get_addr_ip_from_bytes(&payload_bytes, &mut 54);
        let mut addr_trans_ip_bytes: [u8; 16] = [0; 16];
        addr_trans_ip_bytes[..16].copy_from_slice(&payload_bytes[54..70]);
        // THEN: el vector de addr_trans_ip es el correcto
        assert_eq!(addr_trans_ip_bytes, addr_trans_ip);
    }
    #[test]
    fn get_addr_trans_port_from_payload_bytes_returns_the_correct_u16() {
        // GIVEN: Payload bytes de un mensaje version
        let payload_bytes: [u8; 102] = [
            127, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 253, 244, 83, 100, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 54, 89, 113, 236, 71, 157, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 192, 168, 0, 58, 71, 157, 51, 165, 53,
            24, 235, 29, 226, 36, 15, 47, 83, 97, 116, 111, 115, 104, 105, 58, 50, 51, 46, 48, 46,
            48, 47, 1, 0, 0, 0, 1,
        ];
        // WHEN: se ejecuta la funcion get_addr_port_from_bytes con los bytes pasados por parametro
        let addr_trans_port = get_addr_port_from_bytes(&payload_bytes, &mut 70);
        // THEN: el numero de addr_trans_port es el correcto
        assert_eq!(18333u16, addr_trans_port);
    }
    #[test]
    fn get_nonce_from_payload_bytes_returns_the_correct_u64() {
        // GIVEN: Payload bytes de un mensaje version
        let payload_bytes: [u8; 102] = [
            127, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 253, 244, 83, 100, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 54, 89, 113, 236, 71, 157, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 192, 168, 0, 58, 71, 157, 51, 165, 53,
            24, 235, 29, 226, 36, 15, 47, 83, 97, 116, 111, 115, 104, 105, 58, 50, 51, 46, 48, 46,
            48, 47, 1, 0, 0, 0, 1,
        ];
        // WHEN: se ejecuta la funcion get_nonce_from_bytes con los bytes pasados por parametro
        let nonce = get_nonce_from_bytes(&payload_bytes, &mut 72);
        let mut nonce_bytes: [u8; 8] = [0; 8];
        nonce_bytes[0..8].copy_from_slice(&payload_bytes[72..80]);
        // THEN: el numero de nonce es el correcto
        assert_eq!(u64::from_le_bytes(nonce_bytes), nonce);
    }
    #[test]
    fn get_user_agent_bytes_from_payload_bytes_returns_the_correct_compactsizeuint() {
        // GIVEN: Payload bytes de un mensaje version
        let payload_bytes: [u8; 102] = [
            127, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 253, 244, 83, 100, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 54, 89, 113, 236, 71, 157, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 192, 168, 0, 58, 71, 157, 51, 165, 53,
            24, 235, 29, 226, 36, 16, 47, 83, 97, 116, 111, 115, 104, 105, 58, 50, 51, 46, 48, 46,
            48, 47, 1, 0, 0, 0, 1,
        ];
        // WHEN: se ejecuta la funcion get_user_agent_bytes_from_bytes con los bytes pasados por parametro
        let user_agent_bytes = get_user_agent_bytes_from_bytes(&payload_bytes, &mut 80);
        // THEN: el numero de user_agent_bytes es el correcto
        assert_eq!(16u64, user_agent_bytes.decoded_value());
    }
    #[test]
    fn get_user_agent_from_payload_bytes_returns_the_correct_string() -> Result<(), Box<dyn Error>>
    {
        // GIVEN: Payload bytes de un mensaje version
        let payload_bytes: [u8; 102] = [
            127, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 253, 244, 83, 100, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 54, 89, 113, 236, 71, 157, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 192, 168, 0, 58, 71, 157, 51, 165, 53,
            24, 235, 29, 226, 36, 16, 47, 83, 97, 116, 111, 115, 104, 105, 58, 50, 51, 46, 48, 46,
            48, 47, 1, 0, 0, 0, 1,
        ];
        // WHEN: se ejecuta la funcion get_user_agent_from_bytes con los bytes pasados por parametro
        let user_agent = get_user_agent_from_bytes(&payload_bytes, &mut 81, 16u64)?;
        // THEN: el string de user_agent es el correcto
        assert_eq!("/Satoshi:23.0.0/".to_string(), user_agent);
        Ok(())
    }
    #[test]
    fn get_start_height_from_payload_bytes_returns_the_correct_i32() {
        // GIVEN: Payload bytes de un mensaje version
        let payload_bytes: [u8; 102] = [
            127, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 253, 244, 83, 100, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 54, 89, 113, 236, 71, 157, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 192, 168, 0, 58, 71, 157, 51, 165, 53,
            24, 235, 29, 226, 36, 16, 47, 83, 97, 116, 111, 115, 104, 105, 58, 50, 51, 46, 48, 46,
            48, 47, 1, 0, 0, 0, 1,
        ];
        // WHEN: se ejecuta la funcion get_start_height_from_bytes con los bytes pasados por parametro
        let start_height = get_start_height_from_bytes(&payload_bytes, &mut 97);
        // THEN: el numero de star_height es el correcto
        assert_eq!(1i32, start_height);
    }
    #[test]
    fn get_relay_from_payload_bytes_returns_the_correct_bool() {
        // GIVEN: Payload bytes de un mensaje version
        let payload_bytes: [u8; 102] = [
            127, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 253, 244, 83, 100, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 54, 89, 113, 236, 71, 157, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 192, 168, 0, 58, 71, 157, 51, 165, 53,
            24, 235, 29, 226, 36, 16, 47, 83, 97, 116, 111, 115, 104, 105, 58, 50, 51, 46, 48, 46,
            48, 47, 1, 0, 0, 0, 1,
        ];
        // WHEN: se ejecuta la funcion get_relay_from_bytes con los bytes pasados por parametro
        let relay = get_relay_from_bytes(&payload_bytes, 101);
        // THEN: el booleano de relay es el correcto
        assert_eq!(true, relay);
    }
    #[test]
    fn version_payload_to_le_bytes_returns_the_correct_bytes() -> Result<(), Box<dyn Error>> {
        // GIVEN: un struct VersionPayload con todos los campos completos
        let version = 70015;
        let services: u64 = 0;
        let timestamp: i64 = 1683229476; // simulo valor para test
        let addr_recv_service: u64 = 1;
        let socket_addr = "3.34.119.199:18333".to_string().parse()?;
        let addr_recv_ip = get_ipv6_address_ip(socket_addr);
        let addr_recv_port: u16 = 18333;
        let addr_trans_service: u64 = 0;
        let addr_trans_ip = get_ipv6_address_ip("192.168.0.58:52417".to_string().parse()?);
        let addr_trans_port: u16 = 18333;
        let nonce: u64 = 7954216226337911560; // simulo valor para test
        let user_agent_bytes: CompactSizeUint = CompactSizeUint::new(16u128);
        let user_agent: String = "/Satoshi:23.0.0/".to_string();
        let start_height: i32 = 1;
        let relay: bool = true;
        let version_payload = VersionPayload {
            version,
            services,
            timestamp,
            addr_recv_service,
            addr_recv_ip,
            addr_recv_port,
            addr_trans_service,
            addr_trans_ip,
            addr_trans_port,
            nonce,
            user_agent_bytes,
            user_agent,
            start_height,
            relay,
        };
        // WHEN: serializo los campos del struct VersionPayload segun protocolo bitcoin
        let version_payload_bytes = version_payload.to_le_bytes();
        // THEN: obtengo los bytes en el orden y posicion correcta para poder ser enviados junto al header del mensaje
        let expected_bytes: Vec<u8> = vec![
            127, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 36, 11, 84, 100, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 3, 34, 119, 199, 71, 157, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 192, 168, 0, 58, 71, 157, 8, 243, 132,
            189, 131, 13, 99, 110, 16, 47, 83, 97, 116, 111, 115, 104, 105, 58, 50, 51, 46, 48, 46,
            48, 47, 1, 0, 0, 0, 1,
        ];
        assert_eq!(expected_bytes, version_payload_bytes);
        Ok(())
    }
    #[test]
    fn get_ipv6_address_ip_returns_a_correct_vector_of_16_bytes_representing_ipv6_address_ip(
    ) -> Result<(), Box<dyn Error>> {
        // GIVEN: a String representing an address ip
        let add_ip = "3.34.119.199:18333".to_string();
        // WHEN: se ejecuta la funcion get_ipv6_address_ip pasandole el socket address del string
        let ipv6_add_ip = get_ipv6_address_ip(add_ip.parse()?);
        // THEN: devuelve un vector de 16 bytes que representa a la direccion ip serializada segun protocolo bitcoin
        let expected_bytes: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 3, 34, 119, 199];
        assert_eq!(expected_bytes, ipv6_add_ip);
        Ok(())
    }
}
