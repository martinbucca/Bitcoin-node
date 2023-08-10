use bitcoin_hashes::{sha256d, Hash};
use chrono::{TimeZone, Utc, DateTime, Local};

#[derive(Debug, PartialEq, Clone, Copy)]
/// Represents the Block Header of the bitcoin protocol.
pub struct BlockHeader {
    pub version: i32,
    pub previous_block_header_hash: [u8; 32],
    pub merkle_root_hash: [u8; 32],
    pub time: u32,
    pub n_bits: u32,
    pub nonce: u32,
}

impl BlockHeader {
    /// Creates a new BlockHeader with the received fields.
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

    /// Receives a byte array, deserializes it and returns the Block Header.
    /// Updates the offset according to the amount of bytes it read from the byte array.
    pub fn unmarshalling(
        block_header_message: &[u8],
        offset: &mut usize,
    ) -> Result<BlockHeader, &'static str> {
        if block_header_message.len() - *offset < 80 {
            return Err(
                "The received bytes are not enough to deserialize the block header message",
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

    /// Converts the Block Header to bytes according to the bitcoin protocol.
    /// Saves those bytes in the vector received by parameter.
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

    /// Returns the hash of the Block Header.
    pub fn hash(&self) -> [u8; 32] {
        let mut block_header_marshaled: Vec<u8> = Vec::new();
        self.marshalling(&mut block_header_marshaled);
        let hash_block = sha256d::Hash::hash(&block_header_marshaled);
        *hash_block.as_byte_array()
    }

    /// Returns a string that represents the block hash in hexadecimal.
    /// The format is the same that is used in web explorers (e.g https://blockstream.info/testnet/)
    /// to show blocks
    pub fn hex_hash(&self) -> String {
        bytes_to_hex_hash(self.hash())
    }

    /// Returns a string that represents the merkle root hash in hexadecimal.
    /// The format is the same that is used in web explorers (e.g https://blockstream.info/testnet/)
    pub fn hex_merkle_root_hash(&self) -> String {
        bytes_to_hex_hash(self.merkle_root_hash)
    }

    /// Makes the proof of work. Validates the Block Header.
    /// Returns true or false according to whether the validation passes or not.
    pub fn validate(&self) -> bool {
        let n_bits_bytes = self.n_bits.to_be_bytes();
        let mut mantissa = Vec::new();
        mantissa.extend_from_slice(&n_bits_bytes[1..4]);
        let first_byte: u8 = n_bits_bytes[0];
        if first_byte > 32 {
            return false;
        }
        let initial_mantissa_position = 32 - first_byte;
        let mut target: [u8; 32] = [0; 32];
        for i in 0..3 {
            target[(initial_mantissa_position as usize) + i] = mantissa[i];
        }

        let mut block_hash: [u8; 32] = self.hash();
        block_hash.reverse();
        if block_hash < target {
            return true;
        }
        false
    }

    /// Compares the merkle root hash of the block with the received hash.
    pub fn is_same_merkle_root_hash(&self, received_hash: &[u8; 32]) -> bool {
        self.merkle_root_hash == *received_hash
    }

    /// Returns a string that represents the timestamp of the block in local date format
    pub fn local_time(&self) -> String {
        local_time_to_string(self.time as i64)
    }
}
/// Receives an i64 that represents the time in UTC and returns a String that represents the time in local format.
fn local_time_to_string(time: i64) -> String {
    let dt_utc = Utc.timestamp_opt(time, 0).unwrap();
    let dt_local: DateTime<_> = Utc.from_utc_datetime(&dt_utc.naive_utc()).with_timezone(&Local);
    dt_local.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Converts a vector of bytes to a string that represents the hash in hexadecimal.
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

    /// Auxiliary function that initializes a Block Header
    fn generate_block_header() -> Result<BlockHeader, &'static str> {
        let mut message_header: Vec<u8> = Vec::new();
        for i in 0..80 {
            message_header.push(i as u8);
        }
        let mut offset: usize = 0;
        let block_header = BlockHeader::unmarshalling(&message_header, &mut offset)?;
        Ok(block_header)
    }

    #[test]
    fn test_deserialization_of_header_generates_expected_version() -> Result<(), &'static str> {
        let block_header: BlockHeader = generate_block_header()?;
        let expected_value = 0x3020100;
        assert_eq!(block_header.version, expected_value);
        Ok(())
    }

    #[test]
    fn test_deserialization_of_header_generates_expected_previous_block_header_hash(
    ) -> Result<(), &'static str> {
        let block_header: BlockHeader = generate_block_header()?;
        let expected_value = [
            4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
            27, 28, 29, 30, 31, 32, 33, 34, 35,
        ];
        assert_eq!(block_header.previous_block_header_hash, expected_value);
        Ok(())
    }

    #[test]
    fn test_deserialization_of_header_generates_expected_merkle_root_hash() -> Result<(), &'static str>
    {
        let block_header: BlockHeader = generate_block_header()?;
        let expected_value = [
            36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57,
            58, 59, 60, 61, 62, 63, 64, 65, 66, 67,
        ];
        assert_eq!(block_header.merkle_root_hash, expected_value);
        Ok(())
    }

    #[test]
    fn test_deserialization_of_header_generates_expected_time() -> Result<(), &'static str> {
        let block_header: BlockHeader = generate_block_header()?;
        let expected_value = 0x47464544;
        assert_eq!(block_header.time, expected_value);
        Ok(())
    }

    #[test]
    fn test_deserialization_of_header_generates_expected_nbits() -> Result<(), &'static str> {
        let block_header: BlockHeader = generate_block_header()?;
        let expected_value = 0x4B4A4948;
        assert_eq!(block_header.n_bits, expected_value);
        Ok(())
    }

    #[test]
    fn test_deserialization_of_header_generates_expected_nonce() -> Result<(), &'static str> {
        let block_header: BlockHeader = generate_block_header()?;
        let expected_value = 0x4F4E4D4C;
        assert_eq!(block_header.nonce, expected_value);
        Ok(())
    }

    #[test]
    fn test_successful_serialization_of_version_field() -> Result<(), &'static str> {
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
    fn test_successful_serialization_of_previous_block_header_hash_field() -> Result<(), &'static str>
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
    fn test_successful_serialization_of_merkle_root_hash_field() -> Result<(), &'static str> {
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
    fn test_successful_serialization_of_time_field() -> Result<(), &'static str> {
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
    fn test_successful_serialization_of_nbits_field() -> Result<(), &'static str> {
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
    fn test_successful_serialization_of_nonce_field() -> Result<(), &'static str> {
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
    fn test_header_is_hashed_correctly() {
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
    fn test_validate_valid_block_returns_true() {
        let block: BlockHeader = BlockHeader::new(0, [0; 32], [0; 32], 0, 0x20ffffff, 0);
        assert!(block.validate())
    }

    #[test]
    fn test_validate_invalid_block_returns_false() {
        let block: BlockHeader = BlockHeader::new(0, [0; 32], [0; 32], 0, 0x10ffffff, 0);
        assert!(!block.validate())
    }
}
