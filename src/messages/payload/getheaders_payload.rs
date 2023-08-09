use std::error::Error;

use crate::compact_size_uint::CompactSizeUint;

const SIZE_OF_HASH: usize = 32;

#[derive(Clone, Debug)]
/// Representa el payload del mensaje getheaders segun el protocolo de bitcoin
pub struct GetHeadersPayload {
    pub version: u32, // The protocol version
    pub hash_count: CompactSizeUint,
    pub locator_hashes: Vec<[u8; SIZE_OF_HASH]>, // Locator hashes — ordered newest to oldest. The remote peer will reply with its longest known chain, starting from a locator hash if possible and block 1 otherwise.
    pub stop_hash: [u8; SIZE_OF_HASH], // References the header to stop at, or zero to just fetch the maximum 2000 headers
}

impl GetHeadersPayload {
    /// Dado un struct del tipo GetHeadersPayload serializa el payload a bytes segun el protocolo de bitcoin
    /// y devuelve un vetor de bytes que representan el payload del mensaje getheaders
    pub fn to_le_bytes(&self) -> Vec<u8> {
        let mut getheaders_payload_bytes: Vec<u8> = vec![];
        getheaders_payload_bytes.extend_from_slice(&self.version.to_le_bytes());
        getheaders_payload_bytes.extend_from_slice(&self.hash_count.marshalling());
        for hash in &self.locator_hashes {
            getheaders_payload_bytes.extend(hash);
        }
        getheaders_payload_bytes.extend(self.stop_hash);
        getheaders_payload_bytes
    }
    /// Dado un vector de bytes, intenta interpretar el mismo como un payload del mensaje getheaders
    pub fn read_from(payload: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut offset = 0;
        let mut version_bytes: [u8; 4] = [0u8; 4];
        version_bytes.copy_from_slice(&payload[0..4]);
        let version = u32::from_le_bytes(version_bytes);
        offset += 4;
        let hash_count = CompactSizeUint::unmarshalling(payload, &mut offset)?;
        let mut locator_hashes: Vec<[u8; SIZE_OF_HASH]> = vec![];
        for _ in 0..hash_count.decoded_value() {
            let mut hash: [u8; SIZE_OF_HASH] = [0u8; SIZE_OF_HASH];
            hash.copy_from_slice(&payload[offset..offset + SIZE_OF_HASH]);
            locator_hashes.push(hash);
            offset += SIZE_OF_HASH;
        }
        let mut stop_hash: [u8; SIZE_OF_HASH] = [0u8; SIZE_OF_HASH];
        stop_hash.copy_from_slice(&payload[offset..offset + SIZE_OF_HASH]);
        Ok(GetHeadersPayload {
            version,
            hash_count,
            locator_hashes,
            stop_hash,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn getheaders_payload_returns_the_correct_bytes_when_marshalling_to_bytes_with_one_locator_hash(
    ) {
        // GIVEN : un payload del mensaje getheaders en forma de struct GetHeadersPayload con un solo locator hash (genesis)
        let getheaders_payload = GetHeadersPayload {
            version: 70015,
            hash_count: CompactSizeUint::new(1u128),
            locator_hashes: vec![[
                0x00, 0x00, 0x00, 0x00, 0x09, 0x33, 0xea, 0x01, 0xad, 0x0e, 0xe9, 0x84, 0x20, 0x97,
                0x79, 0xba, 0xae, 0xc3, 0xce, 0xd9, 0x0f, 0xa3, 0xf4, 0x08, 0x71, 0x95, 0x26, 0xf8,
                0xd7, 0x7f, 0x49, 0x43,
            ]],
            stop_hash: [0; 32],
        };
        // WHEN: se llama al metodo para serializar el mensaje "to_le_bytes()"
        let bytes = getheaders_payload.to_le_bytes();
        // THEN: se obtienen los bytes esperado
        let expected_bytes: Vec<u8> = vec![
            127, 17, 1, 0, 1, 0, 0, 0, 0, 9, 51, 234, 1, 173, 14, 233, 132, 32, 151, 121, 186, 174,
            195, 206, 217, 15, 163, 244, 8, 113, 149, 38, 248, 215, 127, 73, 67, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(expected_bytes, bytes);
    }
    #[test]
    fn getheaders_payload_returns_the_correct_bytes_when_marshalling_to_bytes_with_more_than_one_locator_hashes(
    ) {
        // GIVEN : un payload del mensaje getheaders en forma de struct GetHeadersPayload con mas de un locator hash
        let getheaders_payload = GetHeadersPayload {
            version: 70015,
            hash_count: CompactSizeUint::new(2u128),
            locator_hashes: vec![
                [
                    0x00, 0x00, 0x00, 0x00, 0x09, 0x33, 0xea, 0x01, 0xad, 0x0e, 0xe9, 0x84, 0x20,
                    0x97, 0x79, 0xba, 0xae, 0xc3, 0xce, 0xd9, 0x0f, 0xa3, 0xf4, 0x08, 0x71, 0x95,
                    0x26, 0xf8, 0xd7, 0x7f, 0x49, 0x43,
                ],
                [
                    0x00, 0x00, 0x00, 0x00, 0x09, 0x33, 0xea, 0x01, 0xad, 0x0e, 0xe9, 0x84, 0x20,
                    0x97, 0x79, 0xba, 0xae, 0xc3, 0xce, 0xd9, 0x0f, 0xa3, 0xf4, 0x08, 0x71, 0x95,
                    0x26, 0xf8, 0xd7, 0x7f, 0x49, 0x44,
                ],
            ],
            stop_hash: [0; 32],
        };
        // WHEN: se llama al metodo para serializar el mensaje "to_le_bytes()"
        let bytes = getheaders_payload.to_le_bytes();
        // THEN: se obtienen los bytes esperado
        let expected_bytes: Vec<u8> = vec![
            127, 17, 1, 0, 2, 0, 0, 0, 0, 9, 51, 234, 1, 173, 14, 233, 132, 32, 151, 121, 186, 174,
            195, 206, 217, 15, 163, 244, 8, 113, 149, 38, 248, 215, 127, 73, 67, 0, 0, 0, 0, 9, 51,
            234, 1, 173, 14, 233, 132, 32, 151, 121, 186, 174, 195, 206, 217, 15, 163, 244, 8, 113,
            149, 38, 248, 215, 127, 73, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(expected_bytes, bytes);
    }
    #[test]
    fn getheaders_payload_returns_the_correct_bytes_when_marshalling_to_bytes_with_more_than_one_locator_hashes_and_stop_hash(
    ) {
        // GIVEN : un payload del mensaje getheaders en forma de struct GetHeadersPayload con mas de un locator hash y un stop hash
        let getheaders_payload = GetHeadersPayload {
            version: 70015,
            hash_count: CompactSizeUint::new(2u128),
            locator_hashes: vec![
                [
                    0x00, 0x00, 0x00, 0x00, 0x09, 0x33, 0xea, 0x01, 0xad, 0x0e, 0xe9, 0x84, 0x20,
                    0x97, 0x79, 0xba, 0xae, 0xc3, 0xce, 0xd9, 0x0f, 0xa3, 0xf4, 0x08, 0x71, 0x95,
                    0x26, 0xf8, 0xd7, 0x7f, 0x49, 0x43,
                ],
                [
                    0x00, 0x00, 0x00, 0x00, 0x09, 0x33, 0xea, 0x01, 0xad, 0x0e, 0xe9, 0x84, 0x20,
                    0x97, 0x79, 0xba, 0xae, 0xc3, 0xce, 0xd9, 0x0f, 0xa3, 0xf4, 0x08, 0x71, 0x95,
                    0x26, 0xf8, 0xd7, 0x7f, 0x49, 0x44,
                ],
            ],
            stop_hash: [
                0x00, 0x00, 0x00, 0x00, 0x09, 0x33, 0xea, 0x01, 0xad, 0x0e, 0xe9, 0x84, 0x20, 0x97,
                0x79, 0xba, 0xae, 0xc3, 0xce, 0xd9, 0x0f, 0xa3, 0xf4, 0x08, 0x71, 0x95, 0x26, 0xf8,
                0xd7, 0x7f, 0x49, 0x45,
            ],
        };
        // WHEN: se llama al metodo para serializar el mensaje "to_le_bytes()"
        let bytes = getheaders_payload.to_le_bytes();
        // THEN: se obtienen los bytes esperado
        let expected_bytes: Vec<u8> = vec![
            127, 17, 1, 0, 2, 0, 0, 0, 0, 9, 51, 234, 1, 173, 14, 233, 132, 32, 151, 121, 186, 174,
            195, 206, 217, 15, 163, 244, 8, 113, 149, 38, 248, 215, 127, 73, 67, 0, 0, 0, 0, 9, 51,
            234, 1, 173, 14, 233, 132, 32, 151, 121, 186, 174, 195, 206, 217, 15, 163, 244, 8, 113,
            149, 38, 248, 215, 127, 73, 68, 0, 0, 0, 0, 9, 51, 234, 1, 173, 14, 233, 132, 32, 151,
            121, 186, 174, 195, 206, 217, 15, 163, 244, 8, 113, 149, 38, 248, 215, 127, 73, 69,
        ];
        assert_eq!(expected_bytes, bytes);
    }
}
