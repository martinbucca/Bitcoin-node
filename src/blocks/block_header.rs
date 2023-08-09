use bitcoin_hashes::{sha256d, Hash};
use chrono::{TimeZone, Utc, DateTime, Local};

/// Representa el Block Header del protocolo bitcoin
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct BlockHeader {
    pub version: i32,
    pub previous_block_header_hash: [u8; 32],
    pub merkle_root_hash: [u8; 32],
    pub time: u32,
    pub n_bits: u32,
    pub nonce: u32,
}

impl BlockHeader {
    /// Inicializa el BlockHeader con los campos recibidos.
    pub fn new(
        version: i32,
        previous_block_header_hash: [u8; 32],
        merkle_root_hash: [u8; 32],
        time: u32,
        n_bits: u32,
        nonce: u32,
    ) -> BlockHeader {
        BlockHeader {
            version,
            previous_block_header_hash,
            merkle_root_hash,
            time,
            n_bits,
            nonce,
        }
    }

    /// Recibe una cadena de bytes, la deserializa y devuelve el Block Header.
    /// Actualiza el offset según la cantidad de bytes que leyó de la cadena.
    pub fn unmarshalling(
        block_header_message: &[u8],
        offset: &mut usize,
    ) -> Result<BlockHeader, &'static str> {
        if block_header_message.len() - *offset < 80 {
            return Err(
                "Los bytes recibidos no corresponden a un BlockHeader, el largo es menor a 80 bytes",
            );
        }
        let mut version_bytes: [u8; 4] = [0; 4];
        version_bytes.copy_from_slice(&block_header_message[*offset..(*offset + 4)]);
        *offset += 4;
        let version = i32::from_le_bytes(version_bytes);
        let mut previous_block_header_hash: [u8; 32] = [0; 32];
        previous_block_header_hash.copy_from_slice(&block_header_message[*offset..(*offset + 32)]);
        *offset += 32;
        let mut merkle_root_hash: [u8; 32] = [0; 32];
        merkle_root_hash.copy_from_slice(&block_header_message[*offset..(*offset + 32)]);
        *offset += 32;
        let mut time_bytes: [u8; 4] = [0; 4];
        time_bytes.copy_from_slice(&block_header_message[*offset..(*offset + 4)]);
        *offset += 4;
        let time = u32::from_le_bytes(time_bytes);
        let mut n_bits_bytes: [u8; 4] = [0; 4];
        n_bits_bytes.copy_from_slice(&block_header_message[*offset..(*offset + 4)]);
        *offset += 4;
        let n_bits = u32::from_le_bytes(n_bits_bytes);
        let mut nonce_bytes: [u8; 4] = [0; 4];
        nonce_bytes.copy_from_slice(&block_header_message[*offset..(*offset + 4)]);
        let nonce = u32::from_le_bytes(nonce_bytes);
        *offset += 4;
        Ok(BlockHeader {
            version,
            previous_block_header_hash,
            merkle_root_hash,
            time,
            n_bits,
            nonce,
        })
    }

    /// Convierte el Block Header a bytes según el protocolo bitcoin.
    /// Guarda dichos bytes en el vector recibido por parámetro.
    pub fn marshalling(&self, marshaled_block_header: &mut Vec<u8>) {
        let version_bytes = self.version.to_le_bytes();
        marshaled_block_header.extend_from_slice(&version_bytes);
        marshaled_block_header.extend_from_slice(&self.previous_block_header_hash);
        marshaled_block_header.extend_from_slice(&self.merkle_root_hash);
        let time_bytes = self.time.to_le_bytes();
        marshaled_block_header.extend_from_slice(&time_bytes);
        let n_bits_bytes = self.n_bits.to_le_bytes();
        marshaled_block_header.extend_from_slice(&n_bits_bytes);
        let nonce_bytes = self.nonce.to_le_bytes();
        marshaled_block_header.extend_from_slice(&nonce_bytes);
    }

    /// Devuelve el hash del Block Header
    pub fn hash(&self) -> [u8; 32] {
        let mut block_header_marshaled: Vec<u8> = Vec::new();
        self.marshalling(&mut block_header_marshaled);
        let hash_block = sha256d::Hash::hash(&block_header_marshaled);
        *hash_block.as_byte_array()
    }

    /// Devuelve un string que representa el hash del bloque en hexadecimal,
    /// En el formato que se usan los exploradores web como
    /// https://blockstream.info/testnet/ para mostrar bloques
    pub fn hex_hash(&self) -> String {
        bytes_to_hex_hash(self.hash())
    }
    /// Devuelve un string que representa el hash del merkle root en hexadecimal,
    /// En el formato que se usan los exploradores web
    pub fn hex_merkle_root_hash(&self) -> String {
        bytes_to_hex_hash(self.merkle_root_hash)
    }

    /// Esta funcion realiza la proof of work
    /// Valida el Block Header.
    /// Devuelve true o false según pasa la validación o no.
    pub fn validate(&self) -> bool {
        let n_bits_bytes = self.n_bits.to_be_bytes();
        let mut mantisa = Vec::new();
        mantisa.extend_from_slice(&n_bits_bytes[1..4]);
        let primer_byte: u8 = n_bits_bytes[0];
        if primer_byte > 32 {
            return false;
        }
        let posicion_inicial_mantisa = 32 - primer_byte;
        let mut target: [u8; 32] = [0; 32];
        for i in 0..3 {
            target[(posicion_inicial_mantisa as usize) + i] = mantisa[i];
        }

        let mut block_hash: [u8; 32] = self.hash();
        block_hash.reverse();
        if block_hash < target {
            return true;
        }
        false
    }

    /// Compara la raiz del merkle root
    pub fn is_same_merkle_root_hash(&self, received_hash: &[u8; 32]) -> bool {
        self.merkle_root_hash == *received_hash
    }

    /// Devuelve un string que representa el timestamp del bloque en formato UTC
    pub fn local_time(&self) -> String {
        local_time_to_string(self.time as i64)
    }
}
/// Recibe el tiempo en formato UTC y lo devuelve en formato String
fn local_time_to_string(time: i64) -> String {
    let dt_utc = Utc.timestamp_opt(time, 0).unwrap();
    let dt_local: DateTime<_> = Utc.from_utc_datetime(&dt_utc.naive_utc()).with_timezone(&Local);
    dt_local.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Convierte un vector de bytes a un string que representa el hash en hexadecimal,
fn bytes_to_hex_hash(hash_as_bytes: [u8; 32]) -> String {
    let inverted_hash: [u8; 32] = {
        let mut inverted = [0; 32];
        for (i, byte) in hash_as_bytes.iter().enumerate() {
            inverted[31 - i] = *byte;
        }
        inverted
    };
    let hex_hash = inverted_hash
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect();
    hex_hash
}

#[cfg(test)]
mod tests {
    use super::BlockHeader;
    use bitcoin_hashes::{sha256d, Hash};

    /// Función auxiliar que inicializa un Block Header
    fn generar_block_header() -> Result<BlockHeader, &'static str> {
        let mut message_header: Vec<u8> = Vec::new();
        for i in 0..80 {
            message_header.push(i as u8);
        }
        let mut offset: usize = 0;
        let blockheader = BlockHeader::unmarshalling(&message_header, &mut offset)?;
        Ok(blockheader)
    }

    #[test]
    fn test_deserializacion_del_header_genera_version_esperada() -> Result<(), &'static str> {
        let blockheader: BlockHeader = generar_block_header()?;
        let expected_value = 0x3020100;
        assert_eq!(blockheader.version, expected_value);
        Ok(())
    }

    #[test]
    fn test_deserializacion_del_header_genera_previous_block_header_hash_esperado(
    ) -> Result<(), &'static str> {
        let blockeheader: BlockHeader = generar_block_header()?;
        let expected_value = [
            4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
            27, 28, 29, 30, 31, 32, 33, 34, 35,
        ];
        assert_eq!(blockeheader.previous_block_header_hash, expected_value);
        Ok(())
    }

    #[test]
    fn test_deserializacion_del_header_genera_merkle_root_hash_esperado() -> Result<(), &'static str>
    {
        let blockeheader: BlockHeader = generar_block_header()?;
        let expected_value = [
            36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57,
            58, 59, 60, 61, 62, 63, 64, 65, 66, 67,
        ];
        assert_eq!(blockeheader.merkle_root_hash, expected_value);
        Ok(())
    }

    #[test]
    fn test_deserializacion_del_header_genera_time_esperado() -> Result<(), &'static str> {
        let blockeheader: BlockHeader = generar_block_header()?;
        let expected_value = 0x47464544;
        assert_eq!(blockeheader.time, expected_value);
        Ok(())
    }

    #[test]
    fn test_deserializacion_del_header_genera_nbits_esperado() -> Result<(), &'static str> {
        let blockeheader: BlockHeader = generar_block_header()?;
        let expected_value = 0x4B4A4948;
        assert_eq!(blockeheader.n_bits, expected_value);
        Ok(())
    }

    #[test]
    fn test_deserializacion_del_header_genera_nonce_esperado() -> Result<(), &'static str> {
        let blockeheader: BlockHeader = generar_block_header()?;
        let expected_value = 0x4F4E4D4C;
        assert_eq!(blockeheader.nonce, expected_value);
        Ok(())
    }

    #[test]
    fn test_serializacion_correcta_del_campo_version() -> Result<(), &'static str> {
        let mut block_header_message: Vec<u8> = Vec::new();
        let block = BlockHeader {
            version: 50462976,
            previous_block_header_hash: [0; 32],
            merkle_root_hash: [0; 32],
            time: 0,
            n_bits: 0,
            nonce: 0,
        };
        block.marshalling(&mut block_header_message);
        let mut offset: usize = 0;
        let expected_block = BlockHeader::unmarshalling(&block_header_message, &mut offset)?;
        let expected_value = 0x3020100;
        assert_eq!(expected_block.version, expected_value);
        Ok(())
    }
    #[test]
    fn test_serializacion_correcta_del_campo_previous_block_header_hash() -> Result<(), &'static str>
    {
        let mut block_header_message: Vec<u8> = Vec::new();
        let value = [1; 32];
        let block = BlockHeader {
            version: 0,
            previous_block_header_hash: value,
            merkle_root_hash: [0; 32],
            time: 0,
            n_bits: 0,
            nonce: 0,
        };
        block.marshalling(&mut block_header_message);
        let mut offset: usize = 0;
        let expected_block = BlockHeader::unmarshalling(&block_header_message, &mut offset)?;
        assert_eq!(expected_block.previous_block_header_hash, value);
        Ok(())
    }
    #[test]
    fn test_serializacion_correcta_del_campo_merkle_root_hash() -> Result<(), &'static str> {
        let mut block_header_message: Vec<u8> = Vec::new();
        let value = [1; 32];
        let block = BlockHeader {
            version: 0,
            previous_block_header_hash: [0; 32],
            merkle_root_hash: value,
            time: 0,
            n_bits: 0,
            nonce: 0,
        };
        block.marshalling(&mut block_header_message);
        let mut offset: usize = 0;
        let expected_block = BlockHeader::unmarshalling(&block_header_message, &mut offset)?;
        assert_eq!(expected_block.merkle_root_hash, value);
        Ok(())
    }
    #[test]
    fn test_serializacion_correcta_del_campo_time() -> Result<(), &'static str> {
        let mut block_header_message: Vec<u8> = Vec::new();
        let value = 0x03020100;
        let block = BlockHeader {
            version: 0,
            previous_block_header_hash: [0; 32],
            merkle_root_hash: [0; 32],
            time: value,
            n_bits: 0,
            nonce: 0,
        };
        block.marshalling(&mut block_header_message);
        let mut offset: usize = 0;
        let expected_block = BlockHeader::unmarshalling(&block_header_message, &mut offset)?;
        assert_eq!(expected_block.time, value);
        Ok(())
    }
    #[test]
    fn test_serializacion_correcta_del_campo_nbits() -> Result<(), &'static str> {
        let mut block_header_message: Vec<u8> = Vec::new();
        let value = 0x03020100;
        let block = BlockHeader {
            version: 0,
            previous_block_header_hash: [0; 32],
            merkle_root_hash: [0; 32],
            time: 0,
            n_bits: value,
            nonce: 0,
        };
        block.marshalling(&mut block_header_message);
        let mut offset: usize = 0;
        let expected_block = BlockHeader::unmarshalling(&block_header_message, &mut offset)?;
        assert_eq!(expected_block.n_bits, value);
        Ok(())
    }
    #[test]
    fn test_serializacion_correcta_del_campo_nonce() -> Result<(), &'static str> {
        let mut block_header_message: Vec<u8> = Vec::new();
        let value = 0x03020100;
        let block = BlockHeader {
            version: 0,
            previous_block_header_hash: [0; 32],
            merkle_root_hash: [0; 32],
            time: 0,
            n_bits: 0,
            nonce: value,
        };
        block.marshalling(&mut block_header_message);
        let mut offset: usize = 0;
        let expected_block = BlockHeader::unmarshalling(&block_header_message, &mut offset)?;
        assert_eq!(expected_block.nonce, value);
        Ok(())
    }
    #[test]
    fn test_el_header_es_hasheado_correctamente() {
        let block_header = BlockHeader {
            version: 0x03020100,
            previous_block_header_hash: [0; 32],
            merkle_root_hash: [0; 32],
            time: 0,
            n_bits: 0,
            nonce: 0,
        };
        let mut block_header_message_expected: [u8; 80] = [0; 80];
        for x in 0..4 {
            block_header_message_expected[x] = x as u8;
        }
        let expected_hash = sha256d::Hash::hash(&block_header_message_expected);
        let expected_hash_be = *expected_hash.as_byte_array();
        // let expected_hash_le = BlockHeader::reverse_bytes(&expected_hash_be);
        let mut hash_expected: [u8; 32] = [0; 32];
        hash_expected.copy_from_slice(&expected_hash_be);
        let hash: [u8; 32] = block_header.hash();
        assert_eq!(hash, hash_expected)
    }

    #[test]
    fn test_validate_de_un_bloque_valido_devuelve_true() {
        let block: BlockHeader = BlockHeader::new(0, [0; 32], [0; 32], 0, 0x20ffffff, 0);
        assert!(block.validate())
    }
    #[test]
    fn test_validate_de_un_bloque_valido_devuelve_false() {
        let block: BlockHeader = BlockHeader::new(0, [0; 32], [0; 32], 0, 0x10ffffff, 0);
        assert!(!block.validate())
    }
}
