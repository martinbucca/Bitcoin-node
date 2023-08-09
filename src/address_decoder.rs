use bitcoin_hashes::{ripemd160, Hash};
use k256::sha2::Digest;
use k256::sha2::Sha256;
use secp256k1::SecretKey;
use std::error::Error;
use std::io;

const UNCOMPRESSED_WIF_LEN: usize = 51;
const COMPRESSED_WIF_LEN: usize = 52;
const ADDRESS_LEN: usize = 34;

/// Receives the private key in bytes.
/// Returns the compressed address.
pub fn generate_address(private_key: &[u8]) -> Result<String, Box<dyn Error>> {
    // Applies the ECDSA algorithm to the private key, then to the public key
    let secp: secp256k1::Secp256k1<secp256k1::All> = secp256k1::Secp256k1::new();
    let key = SecretKey::from_slice(private_key)?;
    let public_key: secp256k1::PublicKey = secp256k1::PublicKey::from_secret_key(&secp, &key);
    let public_key_bytes_compressed = public_key.serialize();

    // Applies RIPEMD160(SHA256(ECDSA(public_key)))
    let ripemd160_hash = hash_160(&public_key_bytes_compressed);

    // Add the version byte (0x00) at the beginning of the RIPEMD-160 hash
    let mut extended_hash = vec![0x6f];
    extended_hash.extend_from_slice(&ripemd160_hash);

    // Calculate the checksum (double SHA-256 hash) of the extended hash
    let checksum = Sha256::digest(Sha256::digest(&extended_hash));

    // Add the first 4 bytes of the checksum at the end of the extended hash
    extended_hash.extend_from_slice(&checksum[..4]);

    // Decode the extended hash to base58 format
    let encoded: bs58::encode::EncodeBuilder<&Vec<u8>> = bs58::encode(&extended_hash);
    Ok(encoded.into_string())
}

/// Receives the compressed public key (33 bytes).
/// Applies RIPEMD160(SHA256(ECDSA(public_key))).
pub fn hash_160(public_key_bytes_compressed: &[u8]) -> [u8; 20] {
    let sha256_hash = Sha256::digest(public_key_bytes_compressed);
    *ripemd160::Hash::hash(&sha256_hash).as_byte_array()
}

/// Receives the compressed address.
/// Returns the PubkeyHash. If the address is invalid, returns an error.
pub fn get_pubkey_hash_from_address(address: &str) -> Result<[u8; 20], Box<dyn Error>> {
    // decoded from &str to bytes, from base58 format to bytes
    validate_address(address)?;
    let address_decoded_bytes = bs58::decode(address).into_vec()?;
    let lenght_bytes = address_decoded_bytes.len();
    let mut pubkey_hash: [u8; 20] = [0; 20];

    // the pubkey hash is the one that makes up the address,
    // removes the network byte and the checksum from the end
    pubkey_hash.copy_from_slice(&address_decoded_bytes[1..(lenght_bytes - 4)]);

    Ok(pubkey_hash)
}

/// Returns the compressed public key (33 bytes) from the private key
pub fn get_pubkey_compressed(private_key: &str) -> Result<[u8; 33], Box<dyn Error>> {
    let private_key = decode_wif_private_key(private_key)?;
    let secp: secp256k1::Secp256k1<secp256k1::All> = secp256k1::Secp256k1::new();
    let key: SecretKey = SecretKey::from_slice(&private_key)?;
    let public_key: secp256k1::PublicKey = secp256k1::PublicKey::from_secret_key(&secp, &key);
    Ok(public_key.serialize())
}

/// Receives a bitcoin address.
/// Checks the checksum and returns an error if it is invalid.
pub fn validate_address(address: &str) -> Result<(), Box<dyn Error>> {
    if address.len() != ADDRESS_LEN {
        return Err(Box::new(std::io::Error::new(
            io::ErrorKind::Other,
            "The address is invalid. It has an invalid length.",
        )));
    }
    // Checksum validation: avoids typing errors in the address
    // Calculate the checksum (double SHA-256 hash) of the extended hash
    let address_decoded_bytes = bs58::decode(address).into_vec()?;
    let lenght_bytes = address_decoded_bytes.len();
    let checksum_hash = Sha256::digest(Sha256::digest(
        &address_decoded_bytes[0..(lenght_bytes - 4)],
    ));

    let checksum_address = &address_decoded_bytes[(lenght_bytes - 4)..lenght_bytes];
    if checksum_address != &checksum_hash[..4] {
        return Err(Box::new(std::io::Error::new(
            io::ErrorKind::Other,
            "The address is invalid. The checksum is invalid.",
        )));
    }
    Ok(())
}

/// Receives a private key in bytes and a compressed address.
/// Returns true or false depending on whether they correspond or not.
pub fn validate_address_private_key(
    private_key: &[u8],
    address: &String,
) -> Result<(), Box<dyn Error>> {
    if !generate_address(private_key)?.eq(address) {
        return Err(Box::new(std::io::Error::new(
            io::ErrorKind::Other,
            "The private key does not correspond to the address",
        )));
    }
    Ok(())
}

/// Receives the WIF private key, either in compressed or uncompressed format.
/// Returns the private key in bytes.
pub fn decode_wif_private_key(wif_private_key: &str) -> Result<[u8; 32], Box<dyn Error>> {
    if wif_private_key.len() < UNCOMPRESSED_WIF_LEN || wif_private_key.len() > COMPRESSED_WIF_LEN {
        return Err(Box::new(std::io::Error::new(
            io::ErrorKind::Other,
            "The WIF private key is invalid. It has an invalid length.",
        )));
    }
    // Decode the private key in WIF format
    let decoded = bs58::decode(wif_private_key).into_vec()?;
    let mut vector = vec![];
    if wif_private_key.len() == UNCOMPRESSED_WIF_LEN {
        vector.extend_from_slice(&decoded[1..&decoded.len() - 4]);
    } else {
        vector.extend_from_slice(&decoded[1..&decoded.len() - 5]);
    }

    if vector.len() != 32 {
        return Err(Box::new(std::io::Error::new(
            io::ErrorKind::Other,
            "The WIF private key could not be decoded.",
        )));
    }

    // Obtain the private key of 32 bytes
    let mut private_key_bytes = [0u8; 32];
    private_key_bytes.copy_from_slice(&vector);

    Ok(private_key_bytes)
}

#[cfg(test)]

mod test {
    use super::get_pubkey_hash_from_address;
    use crate::address_decoder::decode_wif_private_key;
    use crate::address_decoder::generate_address;
    use secp256k1::SecretKey;
    use std::error::Error;
    use std::io;

    /// Generates the pubkey hash from the private key
    fn generate_pubkey_hash(private_key: &[u8]) -> Result<[u8; 20], Box<dyn Error>> {
        let secp: secp256k1::Secp256k1<secp256k1::All> = secp256k1::Secp256k1::new();
        let key: SecretKey = SecretKey::from_slice(private_key)?;
        let public_key: secp256k1::PublicKey = secp256k1::PublicKey::from_secret_key(&secp, &key);
        // Apply RIPEMD160(SHA256(ECDSA(public_key)))
        let public_key_compressed = public_key.serialize();

        // Apply hash160
        Ok(super::hash_160(&public_key_compressed))
    }

    /// Converts the received hexadecimal string into bytes
    fn string_to_32_bytes(input: &str) -> Result<[u8; 32], Box<dyn Error>> {
        if input.len() != 64 {
            return Err(Box::new(std::io::Error::new(
                io::ErrorKind::Other,
                "The received string is invalid. It doesn't have the correct length",
            )));
        }

        let mut result = [0; 32];
        for i in 0..32 {
            let byte_str = &input[i * 2..i * 2 + 2];
            result[i] = u8::from_str_radix(byte_str, 16)?;
        }

        Ok(result)
    }

    #[test]
    fn test_decoding_wif_compressed_correctly_generates_private_key(
    ) -> Result<(), Box<dyn Error>> {
        // WIF COMPRESSED
        let wif = "cMoBjaYS6EraKLNqrNN8DvN93Nnt6pJNfWkYM8pUufYQB5EVZ7SR";
        // PRIVATE KEY FROM HEX FORMAT
        let expected_private_key_bytes =
            string_to_32_bytes("066C2068A5B9D650698828A8E39F94A784E2DDD25C0236AB7F1A014D4F9B4B49")?;
        let private_key = decode_wif_private_key(wif)?;

        assert_eq!(private_key.to_vec(), expected_private_key_bytes);
        Ok(())
    }

    #[test]
    fn test_decoding_wif_uncompressed_correctly_generates_private_key(
    ) -> Result<(), Box<dyn Error>> {
        // WIF UNCOMPRESSED
        let wif = "91dkDNCCaMp2f91sVQRGgdZRw1QY4aptaeZ4vxEvuG5PvZ9hftJ";
        // PRIVATE KEY FROM HEX FORMAT
        let expected_private_key_bytes =
            string_to_32_bytes("066C2068A5B9D650698828A8E39F94A784E2DDD25C0236AB7F1A014D4F9B4B49")?;

        let private_key = decode_wif_private_key(wif)?;
        assert_eq!(private_key.to_vec(), expected_private_key_bytes);
        Ok(())
    }

    #[test]
    fn test_address_generation_is_correct() -> Result<(), Box<dyn Error>> {
        let expected_address: &str = "mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV";
        let private_key_wif: &str = "cMoBjaYS6EraKLNqrNN8DvN93Nnt6pJNfWkYM8pUufYQB5EVZ7SR";
        let private_key_bytes = decode_wif_private_key(private_key_wif)?;
        let address = generate_address(&private_key_bytes)?;
        assert_eq!(expected_address, address);
        Ok(())
    }

    #[test]
    fn test_valid_address_decoding_returns_ok() {
        let address = "mpzx6iZ1WX8hLSeDRKdkLatXXPN1GDWVaF";
        let pubkey_hash_expected = get_pubkey_hash_from_address(address);
        assert!(pubkey_hash_expected.is_ok())
    }

    #[test]
    fn test_address_decoding_generates_expected_pubkey() -> Result<(), Box<dyn Error>> {
        let address: &str = "mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV";
        let private_key: &str = "cMoBjaYS6EraKLNqrNN8DvN93Nnt6pJNfWkYM8pUufYQB5EVZ7SR";
        let private_key_bytes = decode_wif_private_key(private_key)?;
        let pubkey_hash_expected = generate_pubkey_hash(&private_key_bytes)?;
        let pubkey_hash_generated = get_pubkey_hash_from_address(address)?;
        assert_eq!(pubkey_hash_expected, pubkey_hash_generated);
        Ok(())
    }

    #[test]
    fn test_pub_key_hash_is_generated_with_correct_length() -> Result<(), Box<dyn Error>> {
        let address = "mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV";
        let pub_key_hash = get_pubkey_hash_from_address(address)?;

        assert_eq!(pub_key_hash.len(), 20);
        Ok(())
    }
    
    #[test]
    fn test_get_pubkey_hash_with_invalid_address_returns_error() -> Result<(), Box<dyn Error>> {
        let address = "1nEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV";
        let pub_key_hash_result = get_pubkey_hash_from_address(address);

        assert!(pub_key_hash_result.is_err());
        Ok(())
    }
}

