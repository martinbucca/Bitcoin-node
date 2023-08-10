use crate::compact_size_uint::CompactSizeUint;

use super::{outpoint::Outpoint, script::sig_script::SigScript};

#[derive(Debug, PartialEq, Clone)]
/// Represents the TxIn of a transaction as the protocol bitcoin indicates.
pub struct TxIn {
    previous_output: Outpoint,
    script_bytes: CompactSizeUint,
    pub height: Option<Vec<u8>>,
    pub signature_script: SigScript,
    sequence: u32,
}

impl TxIn {
    /// Creates the TxIn with the received parameters.
    pub fn new(
        previous_output: Outpoint,
        script_bytes: CompactSizeUint,
        height: Option<Vec<u8>>,
        signature_script: SigScript,
        sequence: u32,
    ) -> Self {
        TxIn {
            previous_output,
            script_bytes,
            height,
            signature_script,
            sequence,
        }
    }

    /// Creates the TxIn incomplete.
    /// It is used when creating a transaction. The signature_script field is empty.
    pub fn incomplete_txin(previous_output: Outpoint) -> TxIn {
        let script_bytes: CompactSizeUint = CompactSizeUint::new(0);
        let height: Option<Vec<u8>> = None;
        let signature_script: SigScript = SigScript::new(vec![]);
        let sequence: u32 = 0xffffffff;
        Self::new(
            previous_output,
            script_bytes,
            height,
            signature_script,
            sequence,
        )
    }
    
    /// Receives a vector of bytes that contains a txin and an offset indicating the position where it begins.
    /// Returns the txin completing the fields according to the bytes read in case everything is fine
    /// and a string indicating the error when something fails. Updates the offset.
    pub fn unmarshalling(bytes: &Vec<u8>, offset: &mut usize) -> Result<TxIn, &'static str> {
        if bytes.len() - *offset < 41 {
            return Err(
                "The bytes received do not correspond to a TxIn, there are not enough bytes",
            );
        }
        let previous_output: Outpoint = Outpoint::unmarshalling(bytes, offset)?;
        let script_bytes: CompactSizeUint = CompactSizeUint::unmarshalling(bytes, offset)?;
        let mut height: Option<Vec<u8>> = None;
        let mut bytes_for_height = 0;
        if previous_output.is_a_coinbase_outpoint() {
            if script_bytes.decoded_value() > 100 {
                return Err(
                    "The bytes received do not correspond to a TxIn, the script bytes are invalid",
                );
            }
            let mut height_bytes: Vec<u8> = Vec::new();
            height_bytes.extend_from_slice(&bytes[*offset..(*offset + 4)]);
            height = Some(height_bytes);
            *offset += 4;
            bytes_for_height = 4;
        }
        let mut signature_script: Vec<u8> = Vec::new();
        let amount_bytes_to_read: usize = script_bytes.decoded_value() as usize;
        signature_script.extend_from_slice(
            &bytes[*offset..(*offset + amount_bytes_to_read - bytes_for_height)],
        );
        *offset += amount_bytes_to_read - bytes_for_height;
        let mut sequence_bytes: [u8; 4] = [0; 4];
        sequence_bytes.copy_from_slice(&bytes[*offset..*offset + 4]);
        *offset += 4;
        let sequence = u32::from_le_bytes(sequence_bytes);
        Ok(TxIn {
            previous_output,
            script_bytes,
            height,
            signature_script: SigScript::new(signature_script),
            sequence,
        })
    }

    /// Unmarshalls the txins received in the bytes chain.
    /// Updates the offset and returns the TxIn vector.
    pub fn unmarshalling_txins(
        bytes: &Vec<u8>,
        amount_txin: u64,
        offset: &mut usize,
    ) -> Result<Vec<TxIn>, &'static str> {
        let mut tx_in_list: Vec<TxIn> = Vec::new();
        let mut i = 0;
        while i < amount_txin {
            tx_in_list.push(Self::unmarshalling(bytes, offset)?);
            i += 1;
        }
        Ok(tx_in_list)
    }

    /// Marshalls the TxIn to bytes according to the bitcoin protocol.
    /// Saves them in the vector received by parameter.
    pub fn marshalling(&self, bytes: &mut Vec<u8>) {
        self.previous_output.marshalling(bytes);
        let script_bytes: Vec<u8> = self.script_bytes.marshalling();
        bytes.extend_from_slice(&script_bytes);
        if self.is_coinbase() {
            if let Some(height) = &self.height {
                bytes.extend_from_slice(height)
            }
        }
        bytes.extend_from_slice(self.signature_script.get_bytes());
        let sequence_bytes: [u8; 4] = self.sequence.to_le_bytes();
        bytes.extend_from_slice(&sequence_bytes);
    }

    /// Returns true or false depending on whether the TxIn is from a coinbase transaction.
    pub fn is_coinbase(&self) -> bool {
        self.height.is_some()
    }

    /// Returns the previous outpoint
    pub fn outpoint(&self) -> Outpoint {
        self.previous_output
    }

    /// Returns the height of the block in which the transaction is located.
    /// If it is a coinbase transaction it returns the height of the block in which it is located.
    /// If it is not a coinbase transaction it returns 0.
    pub fn get_height(&self) -> u32 {
        let mut bytes: Vec<u8> = vec![0];
        let height = &self.height;
        let mut bytes_from_height: Vec<u8>;
        match height {
            Some(value) => bytes_from_height = value.clone(),
            None => return 0,
        }
        bytes_from_height.reverse();
        bytes.extend_from_slice(&bytes_from_height[..bytes_from_height.len() - 1]);
        let mut aux_bytes: [u8; 4] = [0; 4];
        aux_bytes.copy_from_slice(&bytes);
        u32::from_be_bytes(aux_bytes)
    }

    /// Compares the received hash with the previous output hash of the TxIn
    pub fn is_same_hash(&self, hash: &[u8; 32]) -> bool {
        self.previous_output.same_hash(*hash)
    }
    /// Sets the signature script received in bytes format in the TxIn
    pub fn set_signature_script(&mut self, bytes: Vec<u8>) {
        self.script_bytes = CompactSizeUint::new(bytes.len() as u128);
        self.signature_script = SigScript::new(bytes);
    }
    /// Sets the signature script received in SigScript format in the TxIn
    pub fn add(&mut self, signature: SigScript) {
        self.script_bytes = CompactSizeUint::new(signature.get_bytes().len() as u128);
        self.signature_script = signature
    }
    /// Returns the hash of the previous output
    pub fn get_previous_output_hash(&self) -> [u8; 32] {
        self.previous_output.hash()
    }
    /// Returns the index of the previous output
    pub fn get_previous_output_index(&self) -> usize {
        self.previous_output.index()
    }
}
#[cfg(test)]

mod test {
    use super::TxIn;
    use crate::{
        compact_size_uint::CompactSizeUint,
        transactions::{outpoint::Outpoint, script::sig_script::SigScript},
    };

    fn simulate_data_flow(
        tx_id: [u8; 32],
        index: u32,
        compact_size_value: u128,
        height: Option<Vec<u8>>,
        sequence: u32,
    ) -> Vec<u8> {
        let mut bytes_txin: Vec<u8> = Vec::new();
        let previous_output: Outpoint = Outpoint::new(tx_id, index);
        let script_bytes: CompactSizeUint = CompactSizeUint::new(compact_size_value);
        let mut signature_script: Vec<u8> = Vec::new();
        for _x in 0..compact_size_value {
            signature_script.push(1);
        }
        let txin_to_marshalling: TxIn = TxIn {
            previous_output,
            script_bytes,
            height,
            signature_script: SigScript::new(signature_script),
            sequence,
        };
        txin_to_marshalling.marshalling(&mut bytes_txin);
        bytes_txin
    }

    #[test]
    fn test_unmarshalling_invalid_tx_in() {
        let bytes: Vec<u8> = vec![0; 3];
        let mut offset: usize = 0;
        let tx_in = TxIn::unmarshalling(&bytes, &mut offset);
        assert!(tx_in.is_err());
    }

    #[test]
    fn test_unmarshalling_tx_in_returns_expected_outpoint() -> Result<(), &'static str> {
        let mut bytes: Vec<u8> = Vec::new();
        let outpoint: Outpoint = Outpoint::new([1; 32], 0x30201000);
        outpoint.marshalling(&mut bytes);
        let compact_size: CompactSizeUint = CompactSizeUint::new(1);
        bytes.extend_from_slice(&compact_size.marshalling()[0..1]);
        let signature_script: Vec<u8> = vec![1];
        bytes.extend_from_slice(&signature_script[0..1]);
        let sequence: [u8; 4] = [0xff; 4];
        bytes.extend_from_slice(&sequence[0..4]);
        let mut offset: usize = 0;
        let expected_txin: TxIn = TxIn::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(expected_txin.previous_output, outpoint);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_tx_in_returns_expected_script_bytes() -> Result<(), &'static str> {
        let mut bytes: Vec<u8> = Vec::new();
        let outpoint: Outpoint = Outpoint::new([1; 32], 0x30201000);
        outpoint.marshalling(&mut bytes);
        let compact_size: CompactSizeUint = CompactSizeUint::new(1);
        bytes.extend_from_slice(&compact_size.marshalling()[0..1]);
        let signature_script: Vec<u8> = vec![1];
        bytes.extend_from_slice(&signature_script[0..1]);
        let sequence: [u8; 4] = [0xff; 4];
        bytes.extend_from_slice(&sequence[0..4]);
        let mut offset: usize = 0;
        let expected_txin: TxIn = TxIn::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(expected_txin.script_bytes, compact_size);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_tx_in_returns_expected_signature_script() -> Result<(), &'static str> {
        let mut bytes: Vec<u8> = Vec::new();
        let outpoint: Outpoint = Outpoint::new([1; 32], 0x30201000);
        outpoint.marshalling(&mut bytes);
        let compact_size: CompactSizeUint = CompactSizeUint::new(1);
        bytes.extend_from_slice(&compact_size.marshalling()[0..1]);
        let signature_script: Vec<u8> = vec![1];
        bytes.extend_from_slice(&signature_script[0..1]);
        let sequence: [u8; 4] = [0xff; 4];
        bytes.extend_from_slice(&sequence[0..4]);
        let mut offset: usize = 0;
        let expected_txin: TxIn = TxIn::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(
            *expected_txin.signature_script.get_bytes(),
            signature_script
        );
        assert_eq!(offset, 42);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_tx_in_returns_expected_sequence() -> Result<(), &'static str> {
        let mut bytes: Vec<u8> = Vec::new();
        let outpoint: Outpoint = Outpoint::new([1; 32], 0x30201000);
        outpoint.marshalling(&mut bytes);
        let compact_size: CompactSizeUint = CompactSizeUint::new(1);
        bytes.extend_from_slice(&compact_size.marshalling()[0..1]);
        let signature_script: Vec<u8> = vec![1];
        bytes.extend_from_slice(&signature_script[0..1]);
        let sequence: [u8; 4] = [0xff; 4];
        bytes.extend_from_slice(&sequence[0..4]);
        let mut offset: usize = 0;
        let expected_txin: TxIn = TxIn::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(expected_txin.sequence, 0xffffffff);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_tx_in_returns_expected_offset() -> Result<(), &'static str> {
        let mut bytes: Vec<u8> = Vec::new();
        let outpoint: Outpoint = Outpoint::new([1; 32], 0x30201000);
        outpoint.marshalling(&mut bytes);
        let compact_size: CompactSizeUint = CompactSizeUint::new(1);
        bytes.extend_from_slice(&compact_size.marshalling()[0..1]);
        let signature_script: Vec<u8> = vec![1];
        bytes.extend_from_slice(&signature_script[0..1]);
        let sequence: [u8; 4] = [0xff; 4];
        bytes.extend_from_slice(&sequence[0..4]);
        let mut offset: usize = 0;
        let _expected_txin: TxIn = TxIn::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(offset, 42);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_2_tx_in_returns_expected_offset() -> Result<(), &'static str> {
        let mut bytes: Vec<u8> = Vec::new();
        let outpoint_1: Outpoint = Outpoint::new([1; 32], 0x30201000);
        outpoint_1.marshalling(&mut bytes);
        let compact_size_1: CompactSizeUint = CompactSizeUint::new(1);
        bytes.extend_from_slice(&compact_size_1.marshalling()[0..1]);
        let signature_script_1: Vec<u8> = vec![1];
        bytes.extend_from_slice(&signature_script_1[0..1]);
        let sequence_1: [u8; 4] = [0xff; 4];
        bytes.extend_from_slice(&sequence_1[0..4]);
        let outpoint_2: Outpoint = Outpoint::new([1; 32], 0x30201000);
        outpoint_2.marshalling(&mut bytes);
        let compact_size_2: CompactSizeUint = CompactSizeUint::new(1);
        bytes.extend_from_slice(&compact_size_2.marshalling()[0..1]);
        let signature_script_2: Vec<u8> = vec![1];
        bytes.extend_from_slice(&signature_script_2[0..1]);
        let sequence_2: [u8; 4] = [0xff; 4];
        bytes.extend_from_slice(&sequence_2[0..4]);
        let mut offset: usize = 0;
        let _expected_txin: Vec<TxIn> = TxIn::unmarshalling_txins(&bytes, 2, &mut offset)?;
        assert_eq!(offset, 84);
        Ok(())
    }

    #[test]
    fn test_marshalling_tx_in_serializes_previous_outpoint_correctly() -> Result<(), &'static str> {
        let tx_id: [u8; 32] = [1; 32];
        let index: u32 = 0x30201000;
        let bytes_txin: Vec<u8> = simulate_data_flow(tx_id, index, 2, None, 0xffffffff);
        let mut offset: usize = 0;
        let txin_unmarshaled: TxIn = TxIn::unmarshalling(&bytes_txin, &mut offset)?;
        let expected_previous_output: Outpoint = Outpoint::new(tx_id, index);
        assert_eq!(txin_unmarshaled.previous_output, expected_previous_output);
        Ok(())
    }

    #[test]
    fn test_marshalling_tx_in_serializes_script_bytes_correctly() -> Result<(), &'static str> {
        let tx_id: [u8; 32] = [1; 32];
        let index: u32 = 0x30201000;
        let bytes_txin: Vec<u8> = simulate_data_flow(tx_id, index, 2, None, 0xffffffff);
        let mut offset: usize = 0;
        let txin_unmarshaled: TxIn = TxIn::unmarshalling(&bytes_txin, &mut offset)?;
        let expected_script_bytes: CompactSizeUint = CompactSizeUint::new(2);
        assert_eq!(txin_unmarshaled.script_bytes, expected_script_bytes);
        Ok(())
    }

    #[test]
    fn test_marshalling_tx_in_serializes_signature_script_correctly() -> Result<(), &'static str> {
        let tx_id: [u8; 32] = [1; 32];
        let index: u32 = 0x30201000;
        let bytes_txin: Vec<u8> = simulate_data_flow(tx_id, index, 2, None, 0xffffffff);
        let mut offset: usize = 0;
        let txin_unmarshaled: TxIn = TxIn::unmarshalling(&bytes_txin, &mut offset)?;
        let expected_signature_script: Vec<u8> = vec![1, 1];
        assert_eq!(
            *txin_unmarshaled.signature_script.get_bytes(),
            expected_signature_script
        );
        Ok(())
    }

    #[test]
    fn test_marshalling_tx_in_serializes_sequence_correctly() -> Result<(), &'static str> {
        let tx_id: [u8; 32] = [1; 32];
        let index: u32 = 0x30201000;
        let sequence: u32 = 0x302010;
        let bytes_txin: Vec<u8> = simulate_data_flow(tx_id, index, 2, None, sequence);
        let mut offset: usize = 0;
        let txin_unmarshaled: TxIn = TxIn::unmarshalling(&bytes_txin, &mut offset)?;
        assert_eq!(txin_unmarshaled.sequence, sequence);
        Ok(())
    }
}
