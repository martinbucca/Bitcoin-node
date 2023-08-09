#[derive(Clone, Debug, PartialEq)]
/// Represents a variable length integer as used in the bitcoin protocol.
pub struct CompactSizeUint {
    bytes: Vec<u8>,
}

impl CompactSizeUint {
    /// Creates the CompactSize according to the number received
    pub fn new(value: u128) -> Self {
        CompactSizeUint {
            bytes: Self::generate_compact_size_uint(value),
        }
    }

    /// Generates the bytes of the compact size according to the number received.
    fn generate_compact_size_uint(value: u128) -> Vec<u8> {
        if (253..=0xffff).contains(&value) {
            return Self::get_compact_size_uint(0xfd, 3, value);
        }
        if (0x10000..=0xffffffff).contains(&value) {
            return Self::get_compact_size_uint(0xfe, 5, value);
        }
        if (0x100000000..=0xffffffffffffffff).contains(&value) {
            return Self::get_compact_size_uint(0xff, 9, value);
        }
        vec![value as u8]
    }

    /// Returns the CompactSize in bytes format.
    pub fn value(&self) -> &Vec<u8> {
        &self.bytes
    }
    /// Returns the decoded CompactSize in u64 format.
    pub fn decoded_value(&self) -> u64 {
        let mut bytes: [u8; 8] = [0; 8];
        bytes[0] = self.bytes[0];
        if bytes[0] == 0xfd {
            bytes[..2].copy_from_slice(&self.bytes[1..(2 + 1)]);
            return u64::from_le_bytes(bytes);
        }
        if bytes[0] == 0xfe {
            bytes[..4].copy_from_slice(&self.bytes[1..(4 + 1)]);
            return u64::from_le_bytes(bytes);
        }
        if bytes[0] == 0xff {
            bytes[..8].copy_from_slice(&self.bytes[1..(8 + 1)]);
            return u64::from_le_bytes(bytes);
        }
        u64::from_le_bytes(bytes)
    }

    /// Builds the bytes of the compact size, according to the parameters received.
    fn get_compact_size_uint(first_byte: u8, bytes_amount: u8, value: u128) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();
        bytes.push(first_byte);
        let aux_bytes: [u8; 16] = value.to_le_bytes();
        let mut amount: u8 = 1;
        for byte in aux_bytes {
            if amount == bytes_amount {
                break;
            }
            bytes.push(byte);
            amount += 1;
        }
        bytes
    }

    /// Marshalls the CompactSize to bytes and returns them.
    pub fn marshalling(&self) -> Vec<u8> {
        let mut bytes = vec![];
        bytes.extend(self.value());
        bytes
    }

    /// Unmarshalls the CompactSize according to the bytes received and returns it.
    /// Updates the offset.
    pub fn unmarshalling(
        bytes: &[u8],
        offset: &mut usize,
    ) -> Result<CompactSizeUint, &'static str> {
        if bytes.len() - (*offset) < 1 {
            return Err(
                "The received bytes are not enough to unmarshall the CompactSizeUint",
            );
        }
        let first_byte = bytes[*offset];
        *offset += 1;
        let mut value: Vec<u8> = Vec::new();
        value.push(first_byte);
        if first_byte == 0xfd {
            value.extend_from_slice(&bytes[*offset..(*offset + 2)]);
            *offset += 2;
            return Ok(Self { bytes: value });
        }
        if first_byte == 0xfe {
            value.extend_from_slice(&bytes[*offset..(*offset + 4)]);
            *offset += 4;
            return Ok(Self { bytes: value });
        }
        if first_byte == 0xff {
            value.extend_from_slice(&bytes[*offset..(*offset + 8)]);
            *offset += 8;
            return Ok(Self { bytes: value });
        }
        Ok(Self { bytes: value })
    }
}

#[cfg(test)]
mod test {
    use crate::compact_size_uint::CompactSizeUint;

    #[test]
    fn test_number_200_is_represented_as_0x_c8() {
        let value: u128 = 200;
        let returned_value: CompactSizeUint = CompactSizeUint::new(value);
        let expected_value: Vec<u8> = vec![0xC8];
        assert_eq!(*returned_value.value(), expected_value);
    }

    #[test]
    fn test_number_505_is_represented_as_0x_fd_f9_01() {
        let value: u128 = 505;
        let returned_value: CompactSizeUint = CompactSizeUint::new(value);
        let expected_value: Vec<u8> = vec![0xFD, 0xF9, 0x01];
        assert_eq!(*returned_value.value(), expected_value);
    }

    #[test]
    fn test_number_100000_is_represented_as_0x_fe_a0_86_01_00() {
        let value: u128 = 100000;
        let returned_value: CompactSizeUint = CompactSizeUint::new(value);
        let expected_value: Vec<u8> = vec![0xFE, 0xA0, 0x86, 0x01, 0x00];
        assert_eq!(*returned_value.value(), expected_value);
    }

    #[test]
    fn test_number_5000000000_is_represented_as_0x_ff_00_f2_05_2a_01_00_00_00() {
        let value: u128 = 5000000000;
        let returned_value: CompactSizeUint = CompactSizeUint::new(value);
        let expected_value: Vec<u8> = vec![0xFF, 0x00, 0xF2, 0x05, 0x2A, 0x01, 0x00, 0x00, 0x00];
        assert_eq!(*returned_value.value(), expected_value);
    }

    #[test]
    fn test_unmarshalling_of_1_byte_compact_size_is_done_correctly() -> Result<(), &'static str> {
        let serialized_compact_size: Vec<u8> = vec![0x30];
        let mut offset: usize = 0;
        let expected_compact_size: CompactSizeUint =
            CompactSizeUint::unmarshalling(&serialized_compact_size, &mut offset)?;
        assert_eq!(expected_compact_size.bytes, serialized_compact_size);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_of_3_byte_compact_size_is_done_correctly() -> Result<(), &'static str> {
        let serialized_compact_size: Vec<u8> = vec![0xfd, 0x30, 0x20];
        let mut offset: usize = 0;
        let expected_compact_size: CompactSizeUint =
            CompactSizeUint::unmarshalling(&serialized_compact_size, &mut offset)?;
        assert_eq!(expected_compact_size.bytes, serialized_compact_size);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_of_5_byte_compact_size_is_done_correctly() -> Result<(), &'static str> {
        let serialized_compact_size: Vec<u8> = vec![0xFE, 0xA0, 0x86, 0x01, 0x00];
        let mut offset: usize = 0;
        let expected_compact_size: CompactSizeUint =
            CompactSizeUint::unmarshalling(&serialized_compact_size, &mut offset)?;
        assert_eq!(expected_compact_size.bytes, serialized_compact_size);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_of_9_byte_compact_size_is_done_correctly() -> Result<(), &'static str> {
        let serialized_compact_size: Vec<u8> =
            vec![0xFF, 0x00, 0xF2, 0x05, 0x2A, 0x01, 0x00, 0x00, 0x00];
        let mut offset: usize = 0;
        let expected_compact_size: CompactSizeUint =
            CompactSizeUint::unmarshalling(&serialized_compact_size, &mut offset)?;
        assert_eq!(expected_compact_size.bytes, serialized_compact_size);
        Ok(())
    }

    #[test]
    fn test_compact_size_of_value_0x_fd_f9_01_returns_505_when_decoded() {
        let bytes: Vec<u8> = vec![0xfd, 0xf9, 0x01];
        let compact_size: CompactSizeUint = CompactSizeUint { bytes };
        let expected_value: u64 = compact_size.decoded_value();
        assert_eq!(expected_value, 505);
    }

    #[test]
    fn test_compact_size_of_value_0x_c8_returns_200_when_decoded() {
        let bytes: Vec<u8> = vec![0xc8];
        let compact_size: CompactSizeUint = CompactSizeUint { bytes };
        let expected_value: u64 = compact_size.decoded_value();
        assert_eq!(expected_value, 200);
    }

    #[test]
    fn test_compact_size_of_value_0x_fe_a0_86_01_00_returns_100000_when_decoded() {
        let bytes: Vec<u8> = vec![0xFE, 0xA0, 0x86, 0x01, 0x00];
        let compact_size: CompactSizeUint = CompactSizeUint { bytes };
        let expected_value: u64 = compact_size.decoded_value();
        assert_eq!(expected_value, 100000);
    }

    #[test]
    fn test_compact_size_of_value_0x_ff_00_f2_05_2a_01_00_00_00_returns_5000000000_when_decoded() {
        let bytes: Vec<u8> = vec![0xFF, 0x00, 0xF2, 0x05, 0x2A, 0x01, 0x00, 0x00, 0x00];
        let compact_size: CompactSizeUint = CompactSizeUint { bytes };
        let expected_value: u64 = compact_size.decoded_value();
        assert_eq!(expected_value, 5000000000);
    }
}
