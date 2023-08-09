use super::script_opcodes::ScriptOpcodes;
use crate::address_decoder::{self, get_pubkey_hash_from_address};
use std::error::Error;

const BYTES_TO_PUSH: u8 = 20;
//      <Sig> <PubKey> OP_DUP OP_HASH160 <PubkeyHash> OP_EQUALVERIFY OP_CHECKSIG
//
// scriptPubKey: OP_DUP OP_HASH160 <bytes_to_push> <pubKeyHash> OP_EQUALVERIFY OP_CHECKSIG
// HEXA:         0x76   0xA9       <bytes_to_push> <pubKeyHash>  0x88            0xAC
// Largo bytes:  1 + 1 + 1 + 20 + 1 + 1 = 25
// Si una Tx es P2PKH el largo de su pk_script debe ser == 25
// <pubKeyHash>: Son 20 bytes. Es el resultado de aplicar hash160 (sha256 + ripemd160 hash) a la publicKey comprimida SEC

/// Genera el pubkey script a partir de la address comprimida.
pub fn generate_pubkey_script(address: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let pubkey_hash = get_pubkey_hash_from_address(address)?;
    let mut pk_script: Vec<u8> = Vec::new();
    pk_script.push(ScriptOpcodes::OP_DUP);
    pk_script.push(ScriptOpcodes::OP_HASH160);
    pk_script.push(BYTES_TO_PUSH);
    pk_script.extend_from_slice(&pubkey_hash);
    pk_script.push(ScriptOpcodes::OP_EQUALVERIFY);
    pk_script.push(ScriptOpcodes::OP_CHECKSIG);
    Ok(pk_script)
}

/// Recibe el p2pkh_script y el sig_script.
/// Realiza la validación y devuelve true o false
pub fn validate(p2pkh_script: &[u8], sig_script: &[u8]) -> Result<bool, Box<dyn Error>> {
    // scriptSig:   <length sig>     <sig>   <length pubKey>   <pubKey>
    // <pubKey> es la publicKey comprimida SEC (33bytes) del receptor de la tx
    // Largo bytes: 1 + 71 + 1 + 33 = 106
    // el largo de <sig> depende de la llave DER, puede variar entre 71 o 72
    let length_sig = sig_script[0];
    let mut sig_script_pubkey: [u8; 33] = [0; 33];
    sig_script_pubkey
        .copy_from_slice(&sig_script[length_sig as usize + 2..length_sig as usize + 35]);

    // 1) Chequeo que el primer comando sea OP_DUP (0x76)
    if p2pkh_script[0..1] != [ScriptOpcodes::OP_DUP] {
        return Ok(false);
    }

    // 2) Chequeo que el siguiente comando sea OP_HASH_160 (0xA9)
    if p2pkh_script[1..2] != [ScriptOpcodes::OP_HASH160] {
        return Ok(false);
    }

    // 3) Aplica hash160 sobre el pubkey del sig_script
    let ripemd160_hash = address_decoder::hash_160(&sig_script_pubkey);

    // 4) Chequeo que el siguiente comando sea OP_EQUALVERIFY (0x88)
    if p2pkh_script[23..24] != [ScriptOpcodes::OP_EQUALVERIFY] {
        return Ok(false);
    }

    // 5) Chequeo que los hash coincidan
    if p2pkh_script[3..23] != ripemd160_hash {
        return Ok(false);
    }

    // 6) Chequeo que el siguiente comando sea OP_CHECKSIG (0xAC)
    if p2pkh_script[24..25] != [ScriptOpcodes::OP_CHECKSIG] {
        return Ok(false);
    }
    Ok(true)
}

#[cfg(test)]
mod test {
    use std::error::Error;

    use crate::{
        account::Account,
        transactions::script::{
            p2pkh_script::{self, generate_pubkey_script},
            sig_script::SigScript,
        },
    };

    #[test]
    fn test_pk_script_se_genera_con_el_largo_correcto() -> Result<(), Box<dyn Error>> {
        let address = "mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV";
        let pk_script = generate_pubkey_script(address)?;

        assert_eq!(pk_script.len(), 25);
        Ok(())
    }

    #[test]
    fn test_pk_script_se_genera_con_el_contenido_correcto() -> Result<(), Box<dyn Error>> {
        let address = "mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV";
        let pk_script = generate_pubkey_script(address)?;

        assert_eq!(pk_script[..1], [0x76]);
        assert_eq!(pk_script[1..2], [0xA9]);
        assert_eq!(pk_script[2..3], [20]);
        assert_eq!(pk_script[23..24], [0x88]);
        assert_eq!(pk_script[24..25], [0xAC]);
        Ok(())
    }

    #[test]
    fn test_p2pkh_script_se_valida_correctamente() -> Result<(), Box<dyn Error>> {
        let hash: [u8; 32] = [123; 32];

        let address = "mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV";
        let private_key: String =
            String::from("cMoBjaYS6EraKLNqrNN8DvN93Nnt6pJNfWkYM8pUufYQB5EVZ7SR");
        let account = Account::new(private_key, address.to_string())?;

        let p2pkh_script = generate_pubkey_script(address)?;
        let sig = SigScript::generate_sig_script(hash, &account)?;
        let validation = p2pkh_script::validate(&p2pkh_script, sig.get_bytes())?;

        assert!(validation);
        Ok(())
    }
}
