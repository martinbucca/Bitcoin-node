use crate::account::Account;
use k256::ecdsa;
use k256::elliptic_curve;
use k256::schnorr::signature::SignatureEncoding;
use k256::schnorr::signature::Signer;
use k256::schnorr::signature::Verifier;
use std::error::Error;
#[derive(Debug, PartialEq, Clone)]
/// Represents the signature script of a transaction, as defined in the bitcoin protocol.
pub struct SigScript {
    bytes: Vec<u8>,
}

impl SigScript {
    /// Creates a new signature script with the bytes received by parameter.
    pub fn new(bytes: Vec<u8>) -> Self {
        SigScript { bytes }
    }

    /// Returns the bytes of the signature script.
    pub fn get_bytes(&self) -> &Vec<u8> {
        &self.bytes
    }

    /// Receives the hash to sign and the private key.
    /// Returns the signature.
    fn generate_sig(hash: [u8; 32], private_key: [u8; 32]) -> Result<Vec<u8>, Box<dyn Error>> {
        // Signing
        let secret_key = elliptic_curve::SecretKey::from_bytes((&private_key).into())?;
        let signing_key = ecdsa::SigningKey::from(secret_key);
        let signature: ecdsa::Signature = signing_key.sign(&hash);
        let mut signature_bytes: Vec<u8> = signature.to_der().to_vec();
        // byte of SIGHASH_ALL
        signature_bytes.push(0x01);
        Ok(signature_bytes)
    }

    /// Returns the signature script with the compressed public key.
    pub fn generate_sig_script(
        hash_transaction: [u8; 32],
        account: &Account,
    ) -> Result<SigScript, Box<dyn Error>> {
        let mut sig_script_bytes: Vec<u8> = Vec::new();
        let private_key = account.get_private_key()?;
        let sig = Self::generate_sig(hash_transaction, private_key)?;
        let length_sig = sig.len();

        sig_script_bytes.push(length_sig as u8);
        // loads the sig field
        sig_script_bytes.extend_from_slice(&sig);

        let bytes_public_key = account.get_pubkey_compressed()?;
        let length_pubkey = bytes_public_key.len();
        // loads the length of the public key field
        sig_script_bytes.push(length_pubkey as u8);
        // loads the public key compressed (without hashing)
        sig_script_bytes.extend_from_slice(&bytes_public_key);
        let sig_script = Self::new(sig_script_bytes);
        Ok(sig_script)
    }

    /// Receives the hash, sig and public key.
    /// Returns true or false depending if the sig is correct.
    pub fn verify_sig(
        hash: &[u8],
        sig_bytes: &[u8],
        public_key: &[u8],
    ) -> Result<bool, Box<dyn Error>> {
        // removes the byte of SIGHASH_ALL
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
    fn test_script_sig_length_is_71_bytes_with_key_type() -> Result<(), Box<dyn Error>> {
        let hash: [u8; 32] = [123; 32];
        let signing_key: [u8; 32] = [14; 32];

        let sig = SigScript::generate_sig(hash, signing_key)?;
        assert_eq!(sig.len(), 71);
        Ok(())
    }

    #[test]
    fn test_script_sig_length_is_72_bytes_with_another_key_type() -> Result<(), Box<dyn Error>> {
        let hash: [u8; 32] = [123; 32];
        let signing_key: [u8; 32] = [12; 32];

        let sig = SigScript::generate_sig(hash, signing_key)?;
        assert_eq!(sig.len(), 72);
        Ok(())
    }

    #[test]
    fn test_signature_is_generated_correctly() -> Result<(), Box<dyn Error>> {
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

