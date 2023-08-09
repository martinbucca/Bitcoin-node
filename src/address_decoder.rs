use bitcoin_hashes::{ripemd160, Hash};
use k256::sha2::Digest;
use k256::sha2::Sha256;
use secp256k1::SecretKey;
use std::error::Error;
use std::io;

const UNCOMPRESSED_WIF_LEN: usize = 51;
const COMPRESSED_WIF_LEN: usize = 52;
const ADDRESS_LEN: usize = 34;

/// Recibe la private key en bytes.
/// Devuelve la address comprimida
pub fn generate_address(private_key: &[u8]) -> Result<String, Box<dyn Error>> {
    // se aplica el algoritmo de ECDSA a la clave privada , luego
    // a la clave publica
    let secp: secp256k1::Secp256k1<secp256k1::All> = secp256k1::Secp256k1::new();
    let key = SecretKey::from_slice(private_key)?;
    let public_key: secp256k1::PublicKey = secp256k1::PublicKey::from_secret_key(&secp, &key);
    let public_key_bytes_compressed = public_key.serialize();

    // Se aplica RIPEMD160(SHA256(ECDSA(public_key)))
    let ripemd160_hash = hash_160(&public_key_bytes_compressed);

    // Añadir el byte de versión (0x00) al comienzo del hash RIPEMD-160
    let mut extended_hash = vec![0x6f];
    extended_hash.extend_from_slice(&ripemd160_hash);

    // Calcular el checksum (doble hash SHA-256) del hash extendido
    let checksum = Sha256::digest(Sha256::digest(&extended_hash));

    // Añadir los primeros 4 bytes del checksum al final del hash extendido
    extended_hash.extend_from_slice(&checksum[..4]);

    // Codificar el hash extendido en Base58
    let encoded: bs58::encode::EncodeBuilder<&Vec<u8>> = bs58::encode(&extended_hash);
    Ok(encoded.into_string())
}

/// Recibe el public key comprimido (33 bytes)
/// Aplica RIPEMD160(SHA256(ECDSA(public_key)))
pub fn hash_160(public_key_bytes_compressed: &[u8]) -> [u8; 20] {
    let sha256_hash = Sha256::digest(public_key_bytes_compressed);
    *ripemd160::Hash::hash(&sha256_hash).as_byte_array()
}

/// Recibe la address comprimida
/// Devuelve el PubkeyHash
/// Si la address es invalida, devuelve error
pub fn get_pubkey_hash_from_address(address: &str) -> Result<[u8; 20], Box<dyn Error>> {
    //se decodifican de &str a bytes , desde el formate base58  a bytes
    validate_address(address)?;
    let address_decoded_bytes = bs58::decode(address).into_vec()?;
    let lenght_bytes = address_decoded_bytes.len();
    let mut pubkey_hash: [u8; 20] = [0; 20];

    // el pubkey hash es el que compone la address
    // le saco el byte de la red y el checksum del final
    pubkey_hash.copy_from_slice(&address_decoded_bytes[1..(lenght_bytes - 4)]);

    Ok(pubkey_hash)
}

/// Devuelve la clave publica comprimida (33 bytes) a partir de la privada
pub fn get_pubkey_compressed(private_key: &str) -> Result<[u8; 33], Box<dyn Error>> {
    let private_key = decode_wif_private_key(private_key)?;
    let secp: secp256k1::Secp256k1<secp256k1::All> = secp256k1::Secp256k1::new();
    let key: SecretKey = SecretKey::from_slice(&private_key)?;
    let public_key: secp256k1::PublicKey = secp256k1::PublicKey::from_secret_key(&secp, &key);
    Ok(public_key.serialize())
}

/// Recibe una bitcoin address.
/// Revisa el checksum y devuelve error si es inválida.
pub fn validate_address(address: &str) -> Result<(), Box<dyn Error>> {
    if address.len() != ADDRESS_LEN {
        return Err(Box::new(std::io::Error::new(
            io::ErrorKind::Other,
            "La cantidad de caracteres de la address es inválida.",
        )));
    }
    // validacion checksum: evita errores de tipeo en la address
    // Calcular el checksum (doble hash SHA-256) del hash extendido
    let address_decoded_bytes = bs58::decode(address).into_vec()?;
    let lenght_bytes = address_decoded_bytes.len();
    let checksum_hash = Sha256::digest(Sha256::digest(
        &address_decoded_bytes[0..(lenght_bytes - 4)],
    ));

    let checksum_address = &address_decoded_bytes[(lenght_bytes - 4)..lenght_bytes];
    if checksum_address != &checksum_hash[..4] {
        return Err(Box::new(std::io::Error::new(
            io::ErrorKind::Other,
            "La dirección es inválida, falló la validación del checksum",
        )));
    }
    Ok(())
}

/// Recibe una private key en bytes y una address comprimida.
/// Devuelve true o false dependiendo si se corresponden entre si o no.
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

/// Recibe la WIF private key, ya sea en formato comprimido o no comprimido.
/// Devuelve la private key en bytes
pub fn decode_wif_private_key(wif_private_key: &str) -> Result<[u8; 32], Box<dyn Error>> {
    if wif_private_key.len() < UNCOMPRESSED_WIF_LEN || wif_private_key.len() > COMPRESSED_WIF_LEN {
        return Err(Box::new(std::io::Error::new(
            io::ErrorKind::Other,
            "The WIF private key is invalid. It has an invalid length.",
        )));
    }
    // Decodificar la clave privada en formato WIF
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

    // Obtener la clave privada de 32 bytes
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

    /// Genera el pubkey hash a partir de la private key
    fn generate_pubkey_hash(private_key: &[u8]) -> Result<[u8; 20], Box<dyn Error>> {
        let secp: secp256k1::Secp256k1<secp256k1::All> = secp256k1::Secp256k1::new();
        let key: SecretKey = SecretKey::from_slice(private_key)?;
        let public_key: secp256k1::PublicKey = secp256k1::PublicKey::from_secret_key(&secp, &key);
        //  se aplica RIPEMD160(SHA256(ECDSA(public_key)))
        let public_key_compressed = public_key.serialize();

        // Aplica hash160
        Ok(super::hash_160(&public_key_compressed))
    }

    /// Convierte el str recibido en hexadecimal, a bytes
    fn string_to_32_bytes(input: &str) -> Result<[u8; 32], Box<dyn Error>> {
        if input.len() != 64 {
            return Err(Box::new(std::io::Error::new(
                io::ErrorKind::Other,
                "El string recibido es inválido. No tiene el largo correcto",
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
    fn test_decoding_wif_compressed_genera_correctamente_el_private_key(
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
    fn test_decoding_wif_uncompressed_genera_correctamente_el_private_key(
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
    fn test_address_se_genera_correctamente() -> Result<(), Box<dyn Error>> {
        let expected_address: &str = "mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV";
        let private_key_wif: &str = "cMoBjaYS6EraKLNqrNN8DvN93Nnt6pJNfWkYM8pUufYQB5EVZ7SR";
        let private_key_bytes = decode_wif_private_key(private_key_wif)?;
        let address = generate_address(&private_key_bytes)?;
        assert_eq!(expected_address, address);
        Ok(())
    }

    #[test]
    fn test_decodificacion_de_address_valida_devuelve_ok() {
        let address = "mpzx6iZ1WX8hLSeDRKdkLatXXPN1GDWVaF";
        let pubkey_hash_expected = get_pubkey_hash_from_address(address);
        assert!(pubkey_hash_expected.is_ok())
    }

    #[test]
    fn test_decodificacion_de_address_genera_pubkey_esperado() -> Result<(), Box<dyn Error>> {
        let address: &str = "mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV";
        let private_key: &str = "cMoBjaYS6EraKLNqrNN8DvN93Nnt6pJNfWkYM8pUufYQB5EVZ7SR";
        let private_key_bytes = decode_wif_private_key(private_key)?;
        let pubkey_hash_expected = generate_pubkey_hash(&private_key_bytes)?;
        let pubkey_hash_generated = get_pubkey_hash_from_address(address)?;
        assert_eq!(pubkey_hash_expected, pubkey_hash_generated);
        Ok(())
    }

    #[test]
    fn test_pub_key_hash_se_genera_con_el_largo_correcto() -> Result<(), Box<dyn Error>> {
        let address = "mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV";
        let pub_key_hash = get_pubkey_hash_from_address(address)?;

        assert_eq!(pub_key_hash.len(), 20);
        Ok(())
    }
    #[test]
    fn test_get_pubkey_hash_con_direccion_invalida_da_error() -> Result<(), Box<dyn Error>> {
        let address = "1nEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV";
        let pub_key_hash_result = get_pubkey_hash_from_address(address);

        assert!(pub_key_hash_result.is_err());
        Ok(())
    }
}
