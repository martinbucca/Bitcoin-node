use std::{error::Error, io};

use bitcoin_hashes::{sha256d, Hash};

/// Concatenates the hashes received and then hashes them.
pub fn concatenate_and_hash(first_hash: [u8; 32], second_hash: [u8; 32]) -> [u8; 32] {
    let mut hashs_concatenated: [u8; 64] = [0; 64];
    hashs_concatenated[..32].copy_from_slice(&first_hash[..32]);
    hashs_concatenated[32..(32 + 32)].copy_from_slice(&second_hash[..32]);
    *sha256d::Hash::hash(&hashs_concatenated).as_byte_array()
}

/// Makes the proof to verify if a transaction is in a block. Receives the remaining hashes (including the root)
/// to corroborate that the tx is in a block.
pub fn make_merkle_proof(hashes: &Vec<([u8; 32], bool)>, tx_id_to_find: &[u8; 32]) -> bool {
    let root_position = hashes.len() - 1;
    let mut current_tx = *tx_id_to_find;
    let mut index = 0;
    while index < root_position {
        let hash_first = hashes[index].1;
        if hash_first {
            current_tx = concatenate_and_hash(hashes[index].0, current_tx);
        } else {
            current_tx = concatenate_and_hash(current_tx, hashes[index].0);
        }
        index += 1;
    }
    current_tx == hashes[root_position].0
}

/// Converts the str received in hexadecimal, to bytes.
pub fn string_to_bytes(input: &str) -> Result<[u8; 32], Box<dyn Error>> {
    if input.len() != 64 {
        return Err(Box::new(std::io::Error::new(
            io::ErrorKind::Other,
            "Invalid string. The string must be 64 characters long",
        )));
    }

    let mut result = [0; 32];
    for i in 0..32 {
        let byte_str = &input[i * 2..i * 2 + 2];
        result[i] = u8::from_str_radix(byte_str, 16)?;
    }

    Ok(result)
}
