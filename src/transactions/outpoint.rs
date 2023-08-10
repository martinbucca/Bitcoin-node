#[derive(PartialEq, Debug, Copy, Clone)]
/// Represents an outpoint as defined in the bitcoin protocol.
pub struct Outpoint {
    tx_id: [u8; 32],
    index: u32,
}

impl Outpoint {
    /// Creates a new Outpoint with the tx_id and index received.
    pub fn new(tx_id: [u8; 32], index: u32) -> Self {
        Outpoint { tx_id, index }
    }

    /// Checks the outpoint and returns true or false depending on whether it is a coinbase or not.
    pub fn is_a_coinbase_outpoint(&self) -> bool {
        if self.index == 0xffffffff {
            return true;
        }
        let null_hash: [u8; 32] = [0; 32];
        if self.tx_id == null_hash {
            return true;
        }
        false
    }

    /// Receives a byte array, advances reading on it and returns the Outpoint.
    /// Updates the offset.
    pub fn unmarshalling(bytes: &Vec<u8>, offset: &mut usize) -> Result<Outpoint, &'static str> {
        if bytes.len() - *offset < 36 {
            return Err(
                "The bytes array is not long enough to unmarshall an Outpoint. It must be at least 36 bytes long",
            );
        }
        let mut tx_id: [u8; 32] = [0; 32];
        tx_id.copy_from_slice(&bytes[*offset..(*offset + 32)]);
        *offset += 32;
        let mut index_bytes: [u8; 4] = [0; 4];
        index_bytes.copy_from_slice(&bytes[*offset..(*offset + 4)]);
        *offset += 4;
        let index = u32::from_le_bytes(index_bytes);
        Ok(Outpoint { tx_id, index })
    }

    /// Marshalls the Outpoint according to the bitcoin protocol.
    /// It is stored in the received array.  
    pub fn marshalling(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.tx_id[0..32]); // se cargan los elementos del tx_id
        let index_bytes: [u8; 4] = self.index.to_le_bytes();
        for item in index_bytes {
            bytes.push(item);
        }
    }

    /// Compares the received hash with the outpoint's.
    /// Returns true or false depending on whether they match or not.
    pub fn same_hash(&self, hash: [u8; 32]) -> bool {
        self.tx_id == hash
    }

    /// Returns the index of the outpoint.
    pub fn index(&self) -> usize {
        self.index as usize
    }

    /// Returns the hash of the outpoint.
    pub fn hash(&self) -> [u8; 32] {
        self.tx_id
    }
}

#[cfg(test)]

mod test {
    use super::Outpoint;

    #[test]
    fn test_unmarshalling_outpoint_yields_expected_tx_id() -> Result<(), &'static str> {
        let bytes: Vec<u8> = vec![1; 36];
        let expected_tx_id: [u8; 32] = [1; 32];
        let mut offset: usize = 0;
        let outpoint: Outpoint = Outpoint::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(outpoint.tx_id, expected_tx_id);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_outpoint_yields_expected_index() -> Result<(), &'static str> {
        let mut bytes: Vec<u8> = vec![0; 36];
        for x in 0..4 {
            bytes[32 + x] = x as u8;
        }
        let expected_index: u32 = 0x03020100;
        let mut offset: usize = 0;
        let outpoint: Outpoint = Outpoint::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(outpoint.index, expected_index);
        Ok(())
    }

    #[test]
    fn test_marshalling_outpoint_yields_expected_tx_id() -> Result<(), &'static str> {
        let mut marshalled_outpoint: Vec<u8> = Vec::new();
        let tx_id: [u8; 32] = [2; 32];
        let outpoint_to_marshall: Outpoint = Outpoint {
            tx_id,
            index: 0x03020100,
        };
        outpoint_to_marshall.marshalling(&mut marshalled_outpoint);
        let mut offset: usize = 0;
        let unmarshalled_outpoint: Outpoint =
            Outpoint::unmarshalling(&marshalled_outpoint, &mut offset)?;
        assert_eq!(unmarshalled_outpoint.tx_id, tx_id);
        Ok(())
    }

    #[test]
    fn test_marshalling_outpoint_yields_expected_index() -> Result<(), &'static str> {
        let mut marshalled_outpoint: Vec<u8> = Vec::new();
        let tx_id: [u8; 32] = [2; 32];
        let index: u32 = 0x03020100;
        let outpoint_to_marshall: Outpoint = Outpoint { tx_id, index };
        outpoint_to_marshall.marshalling(&mut marshalled_outpoint);
        let mut offset: usize = 0;
        let unmarshalled_outpoint: Outpoint =
            Outpoint::unmarshalling(&marshalled_outpoint, &mut offset)?;
        assert_eq!(unmarshalled_outpoint.index, index);
        Ok(())
    }

    #[test]
    fn test_outpoint_corresponding_to_null_tx_id_coinbase_returns_true() {
        let coinbase_outpoint: Outpoint = Outpoint::new([1; 32], 0xffffffff);
        assert!(coinbase_outpoint.is_a_coinbase_outpoint())
    }

    #[test]
    fn test_outpoint_corresponding_to_index_0xffffffff_coinbase_returns_true() {
        let coinbase_outpoint: Outpoint = Outpoint::new([1; 32], 0xffffffff);
        assert!(coinbase_outpoint.is_a_coinbase_outpoint())
    }
}
