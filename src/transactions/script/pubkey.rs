use super::script_opcodes::ScriptOpcodes;
use k256::sha2::Digest;
use k256::sha2::Sha256;

#[derive(Debug, PartialEq, Clone)]
pub struct Pubkey {
    bytes: Vec<u8>,
}

impl Pubkey {
    pub fn new(bytes: Vec<u8>) -> Self {
        Pubkey { bytes }
    }
    pub fn bytes(&self) -> &Vec<u8> {
        &self.bytes
    }
    /// Genera la address a partir del pubkey.
    pub fn generate_address(&self) -> Result<String, &'static str> {
        // vector que generara el address
        let mut adress_bytes: Vec<u8> = vec![0x6f];
        let bytes = &self.bytes;
        let lenght: usize = bytes.len();
        if lenght <= 3 {
            return Err("El campo pubkey no tiene el largo esperado");
        }

        let first_byte = self.bytes[0];
        if first_byte == 0x00 {
            // se trata de una transanccion del tipo P2WPKH
            adress_bytes.extend_from_slice(&bytes[2..lenght]);
        }
        if first_byte == ScriptOpcodes::OP_DUP {
            // se trata de una transanccion del tipo P2PKH
            adress_bytes.extend_from_slice(&bytes[3..(lenght - 2)]);
        }
        let copy_adress_bytes: Vec<u8> = adress_bytes.clone();
        let checksum = Sha256::digest(Sha256::digest(copy_adress_bytes));
        adress_bytes.extend_from_slice(&checksum[..4]);
        let encoded: bs58::encode::EncodeBuilder<&Vec<u8>> = bs58::encode(&adress_bytes);
        let string = encoded.into_string();
        Ok(string)
    }
}
