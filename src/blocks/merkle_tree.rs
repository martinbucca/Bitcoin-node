use super::utils_block::concatenate_and_hash;

/// Almacena los hashes correspondientes para generar el merkle tree
/// esta en orden invertido el primer nivel son las hojas , el ultimo es la raiz
pub struct MerkleTree {
    hashes: Vec<Vec<[u8; 32]>>,
}

impl MerkleTree {
    pub fn new(hashes: &Vec<[u8; 32]>) -> Self {
        MerkleTree {
            hashes: Self::generate_merkle_tree(hashes),
        }
    }
    /// Sirve para generar los distintos niveles del arbol
    fn recursive_generation_merkle_root(
        vector: Vec<[u8; 32]>,
        merkle_tree: &mut Vec<Vec<[u8; 32]>>,
    ) {
        let vec_length: usize = vector.len();
        if vec_length == 1 {
            return;
        }
        let mut upper_level: Vec<[u8; 32]> = Vec::new();
        let mut amount_hashs: usize = 0;
        let mut current_position: usize = 0;
        for tx in &vector {
            amount_hashs += 1;
            if amount_hashs == 2 {
                upper_level.push(concatenate_and_hash(vector[current_position - 1], *tx));
                amount_hashs = 0;
            }
            current_position += 1;
        }
        // Si el largo del vector es impar el ultimo elelmento debe concatenarse consigo
        // mismo y luego aplicarse la funcion de hash
        if (vec_length % 2) != 0 {
            upper_level.push(concatenate_and_hash(
                vector[current_position - 1],
                vector[current_position - 1],
            ));
        }
        merkle_tree.push(upper_level.clone());
        let lenght_upper_level = upper_level.len();
        if lenght_upper_level == 1 {
            return;
        }
        if lenght_upper_level % 2 != 0 {
            let last_position = merkle_tree.len() - 1;
            merkle_tree[last_position].push(upper_level[lenght_upper_level - 1]);
        }
        Self::recursive_generation_merkle_root(upper_level, merkle_tree)
    }

    /// Crea el merkle tree a partir del vector de hashes recibidos , dentro llama a una
    /// funcion recursiva para generar el arbol
    fn generate_merkle_tree(txs: &Vec<[u8; 32]>) -> Vec<Vec<[u8; 32]>> {
        let mut merkle_transactions: Vec<[u8; 32]> = Vec::new();
        let mut merkle_tree: Vec<Vec<[u8; 32]>> = Vec::new();
        let lenght_txs = txs.len();
        for tx in txs {
            merkle_transactions.push(*tx);
        }
        if lenght_txs % 2 > 0 {
            merkle_transactions.push(merkle_transactions[lenght_txs - 1]);
        }
        merkle_tree.push(merkle_transactions.clone());
        Self::recursive_generation_merkle_root(merkle_transactions, &mut merkle_tree);
        merkle_tree
    }
    /// Devuelve la raiz del arbol
    pub fn get_merkle_root(&self) -> [u8; 32] {
        let root_position = self.hashes.len() - 1;
        self.hashes[root_position][0]
    }

    /// Carga en el parametro path el hash que corresponde segun el indice recibido
    fn get_hash_from_level(
        merkle_tree: &Vec<Vec<[u8; 32]>>,
        path: &mut Vec<([u8; 32], bool)>,
        level: usize,
        index: usize,
    ) {
        if merkle_tree[level].len() == 1 {
            path.push((merkle_tree[level][0], false));
            return;
        }
        if index % 2 == 0 {
            path.push((merkle_tree[level][index + 1], false));
        } else {
            path.push((merkle_tree[level][index - 1], true));
        }
        let next_index: usize = index / 2;
        let next_level: usize = level + 1;
        Self::get_hash_from_level(merkle_tree, path, next_level, next_index)
    }

    /// Devuelve el camino que debe recorrerse para crear la raiz del bloque seleccionado
    /// usa recursividad para obtener los hashes de los niveles superiores
    pub fn merkle_proof_of_inclusion(
        &self,
        tx_id_to_find: [u8; 32],
    ) -> Option<Vec<([u8; 32], bool)>> {
        let mut tx_id_not_finded = true;
        let mut level: usize = 0;
        // primer nivel (hojas)
        let current_level = &self.hashes[level];
        let lenght = current_level.len();
        let mut index = 0;
        // se fija si se encuentra el hash a buscar dentro del nivel
        while index < lenght {
            if current_level[index] == tx_id_to_find {
                tx_id_not_finded = false;
                break;
            }
            index += 1;
        }
        // Si no se encuentra retorna None
        if tx_id_not_finded {
            return None;
        }
        let mut path: Vec<([u8; 32], bool)> = Vec::new();
        if index % 2 == 0 {
            path.push((current_level[index + 1], false));
        } else {
            path.push((current_level[index - 1], true));
        }
        let next_index = index / 2;
        level += 1;
        Self::get_hash_from_level(&self.hashes, &mut path, level, next_index);
        Some(path)
    }
}

#[cfg(test)]
mod test {
    use std::{error::Error, io};

    use crate::blocks::{merkle_tree::MerkleTree, utils_block::make_merkle_proof};

    /// Genera un vector de [u8;32] que representa cada hash asociado a un transaccion
    /// de la testnet
    fn generate_hashes() -> Result<Vec<[u8; 32]>, Box<dyn Error>> {
        let string_hashes: Vec<&str> = vec![
            "3bec0ba7b6a530a33d6f5cec64947ca2bc9c7f15dc7b73a33311203a7c53e629",
            "c03c2aa43ba796a6d381106416acd7b8dc5f8305de3cbf4c659b2bf8bfed0f18",
            "bf0175a17bc77f372657f52c67ea5a18f5b3b0fd04e93a8146fe19b484cb3245",
            "aa87fefe302d1cd0634cb1e73f4371f9786787e4968bf87868f397801489a325",
            "2d1293d2e0d5a018feddf157931e2842a650acfbf5606867cc78adbe5293c1f6",
        ];
        let mut bytes_hashes: Vec<[u8; 32]> = vec![];
        for string in string_hashes {
            let mut vec = string_to_bytes(string)?;
            vec.reverse();
            bytes_hashes.push(vec);
        }
        Ok(bytes_hashes)
    }

    /// Convierte el str recibido en hexadecimal, a bytes
    fn string_to_bytes(input: &str) -> Result<[u8; 32], Box<dyn Error>> {
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
    fn merkle_root_se_genera_corectamente_con_transacciones_de_testnet(
    ) -> Result<(), Box<dyn Error>> {
        let bytes_hashes: Vec<[u8; 32]> = generate_hashes()?;
        let mut merkle_root_expected: [u8; 32] =
            string_to_bytes("50c77c783a4188784c28c135b1f6e37c977931fcadcdeecd8e4130f7c1916d54")?;
        merkle_root_expected.reverse();
        let merkle_tree: MerkleTree = MerkleTree::new(&bytes_hashes);
        assert_eq!(merkle_tree.get_merkle_root(), merkle_root_expected);
        Ok(())
    }

    #[test]
    fn se_genera_correctamente_una_merkle_proof_of_inclusion_con_tx_en_posicion_impar(
    ) -> Result<(), Box<dyn Error>> {
        let txs: Vec<[u8; 32]> = generate_hashes()?;
        let mut tx_id_to_find =
            string_to_bytes("c03c2aa43ba796a6d381106416acd7b8dc5f8305de3cbf4c659b2bf8bfed0f18")?;
        tx_id_to_find.reverse();
        let merkle_tree: MerkleTree = MerkleTree::new(&txs);

        let option: Option<Vec<([u8; 32], bool)>> =
            merkle_tree.merkle_proof_of_inclusion(tx_id_to_find);
        let hashes = match option {
            Some(value) => value,
            None => return Err("la tx no se encuentra en el merkle tree".into()),
        };
        assert!(make_merkle_proof(&hashes, &tx_id_to_find));
        Ok(())
    }

    #[test]
    fn se_genera_correctamente_una_merkle_proof_of_inclusion_con_tx_en_posicion_par(
    ) -> Result<(), Box<dyn Error>> {
        let txs: Vec<[u8; 32]> = generate_hashes()?;
        let mut tx_id_to_find =
            string_to_bytes("3bec0ba7b6a530a33d6f5cec64947ca2bc9c7f15dc7b73a33311203a7c53e629")?;
        tx_id_to_find.reverse();
        let merkle_tree: MerkleTree = MerkleTree::new(&txs);

        let option: Option<Vec<([u8; 32], bool)>> =
            merkle_tree.merkle_proof_of_inclusion(tx_id_to_find);
        let hashes = match option {
            Some(value) => value,
            None => return Err("la tx no se encuentra en el merkle tree".into()),
        };
        assert!(make_merkle_proof(&hashes, &tx_id_to_find));
        Ok(())
    }

    #[test]
    fn merkle_proof_of_inclusion_devuelve_none_al_no_encontrar_la_tx_a_buscar(
    ) -> Result<(), Box<dyn Error>> {
        let txs: Vec<[u8; 32]> = generate_hashes()?;
        let mut tx_id_to_find =
            string_to_bytes("3bec0ba7b6a530a3346f5cec64947ca2bc9c7f15dc7b73a33311203a7c53e629")?;
        tx_id_to_find.reverse();
        let merkle_tree: MerkleTree = MerkleTree::new(&txs);

        let hashes: Option<Vec<([u8; 32], bool)>> =
            merkle_tree.merkle_proof_of_inclusion(tx_id_to_find);
        assert!(hashes.is_none());
        Ok(())
    }
}
