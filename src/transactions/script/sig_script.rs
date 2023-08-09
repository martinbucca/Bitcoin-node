use crate::account::Account;
use k256::ecdsa;
use k256::elliptic_curve;
use k256::schnorr::signature::SignatureEncoding;
use k256::schnorr::signature::Signer;
use k256::schnorr::signature::Verifier;
use std::error::Error;
#[derive(Debug, PartialEq, Clone)]
pub struct SigScript {
    bytes: Vec<u8>,
}

impl SigScript {
    pub fn new(bytes: Vec<u8>) -> Self {
        SigScript { bytes }
    }

    pub fn get_bytes(&self) -> &Vec<u8> {
        &self.bytes
    }

    /// Recibe el hash a firmar y la private key
    /// Devuelve el signature
    fn generate_sig(hash: [u8; 32], private_key: [u8; 32]) -> Result<Vec<u8>, Box<dyn Error>> {
        // Signing
        let secret_key = elliptic_curve::SecretKey::from_bytes((&private_key).into())?;
        let signing_key = ecdsa::SigningKey::from(secret_key);
        let signature: ecdsa::Signature = signing_key.sign(&hash);
        let mut signature_bytes: Vec<u8> = signature.to_der().to_vec();
        // se carga el byte de SIGHASH_ALL
        signature_bytes.push(0x01);
        Ok(signature_bytes)
    }

    /// Devuelve el signature script con la clave publica comprimida
    pub fn generate_sig_script(
        hash_transaction: [u8; 32],
        account: &Account,
    ) -> Result<SigScript, Box<dyn Error>> {
        let mut sig_script_bytes: Vec<u8> = Vec::new();
        let private_key = account.get_private_key()?;
        let sig = Self::generate_sig(hash_transaction, private_key)?;
        let lenght_sig = sig.len();

        sig_script_bytes.push(lenght_sig as u8);
        // se carga el campo sig
        sig_script_bytes.extend_from_slice(&sig);

        let bytes_public_key = account.get_pubkey_compressed()?;
        let lenght_pubkey = bytes_public_key.len();
        // se carga el largo de los bytes de la clave publica
        sig_script_bytes.push(lenght_pubkey as u8);
        // se carga la clave publica comprimida (sin hashear)
        sig_script_bytes.extend_from_slice(&bytes_public_key);
        let sig_script = Self::new(sig_script_bytes);
        Ok(sig_script)
    }

    /// Recive el hash, sig y public key.
    /// Devuelve true o false dependiendo si el sig es correcto.
    pub fn verify_sig(
        hash: &[u8],
        sig_bytes: &[u8],
        public_key: &[u8],
    ) -> Result<bool, Box<dyn Error>> {
        // se saca el byte de SIGHASH_ALL
        let signature_bytes_without_flag = &sig_bytes[0..sig_bytes.len() - 1];
        let verifying_key = ecdsa::VerifyingKey::from_sec1_bytes(public_key)?;
        let signature = ecdsa::Signature::from_der(signature_bytes_without_flag)?;
        Ok(verifying_key.verify(hash, &signature).is_ok())
    }
}
#[cfg(test)]
mod test {
    use std::error::Error;

    use crate::{account::Account, transactions::script::sig_script::SigScript};
    #[test]
    fn test_el_largo_del_script_sig_es_71_bytes_con_un_tipo_de_clave() -> Result<(), Box<dyn Error>>
    {
        let hash: [u8; 32] = [123; 32];
        let signing_key: [u8; 32] = [14; 32];

        let sig = SigScript::generate_sig(hash, signing_key)?;
        assert_eq!(sig.len(), 71);
        Ok(())
    }

    #[test]
    fn test_el_largo_del_script_sig_es_72_bytes_con_otro_tipo_de_clave(
    ) -> Result<(), Box<dyn Error>> {
        let hash: [u8; 32] = [123; 32];
        let signing_key: [u8; 32] = [12; 32];

        let sig = SigScript::generate_sig(hash, signing_key)?;
        assert_eq!(sig.len(), 72);
        Ok(())
    }

    #[test]
    fn test_la_firma_se_realiza_correctamente() -> Result<(), Box<dyn Error>> {
        let hash: [u8; 32] = [123; 32];
        let address_expected: String = String::from("mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV");
        let private_key: String =
            String::from("cMoBjaYS6EraKLNqrNN8DvN93Nnt6pJNfWkYM8pUufYQB5EVZ7SR");
        let account = Account::new(private_key, address_expected)?;
        let sig = SigScript::generate_sig(hash.clone(), account.get_private_key()?)?;
        assert!(SigScript::verify_sig(
            &hash,
            &sig,
            &account.get_pubkey_compressed()?
        )?);
        Ok(())
    }
}
