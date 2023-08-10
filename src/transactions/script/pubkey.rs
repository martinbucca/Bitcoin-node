use super::script_opcodes::ScriptOpcodes;
use k256::sha2::Digest;
use k256::sha2::Sha256;

#[derive(Debug, PartialEq, Clone)]
/// Represents a public key.
pub struct Pubkey {
    bytes: Vec<u8>,
}

impl Pubkey {
    /// Creates a new pubkey.
    pub fn new(bytes: Vec<u8>) -> Self {
        Pubkey { bytes }
    }
    /// Returns the bytes of the pubkey.
    pub fn bytes(&self) -> &Vec<u8> {
        &self.bytes
    }
    /// Generate the address from the pubkey.
    pub fn generate_address(&self) -> Result<String, &'static str> {
        // vec that generates the address
        let mut adress_bytes: Vec<u8> = vec![0x6f];
        let bytes = &self.bytes;
        let length: usize = bytes.len();
        if length <= 3 {
            return Err("The pubkey field is too short");
        }

        let first_byte = self.bytes[0];
        if first_byte == 0x00 {
            // the transaction is of the P2WPKH type
            adress_bytes.extend_from_slice(&bytes[2..length]);
        }
        if first_byte == ScriptOpcodes::OP_DUP {
            // the transaction is of the P2PKH type
            adress_bytes.extend_from_slice(&bytes[3..(length - 2)]);
        }
        let copy_adress_bytes: Vec<u8> = adress_bytes.clone();
        let checksum = Sha256::digest(Sha256::digest(copy_adress_bytes));
        adress_bytes.extend_from_slice(&checksum[..4]);
        let encoded: bs58::encode::EncodeBuilder<&Vec<u8>> = bs58::encode(&adress_bytes);
        let string = encoded.into_string();
        Ok(string)
    }
}
