#[derive(PartialEq, Debug, Copy, Clone)]
pub struct Outpoint {
    tx_id: [u8; 32],
    index: u32,
}

impl Outpoint {
    pub fn new(tx_id: [u8; 32], index: u32) -> Self {
        Outpoint { tx_id, index }
    }

    /// Revisa el outpoint y devuelve true o false dependiendo si se trata de una coinbase o no.
    pub fn is_a_coinbase_outpoint(&self) -> bool {
        if self.index == 0xffffffff {
            return true;
        }
        let null_hash: [u8; 32] = [0; 32];
        if self.tx_id == null_hash {
            return true;
        }
        false
    }

    /// Recibe una cadena de bytes, avanza leyendo sobre la misma y devuelve el Outpoint.
    /// Actualiza el offset
    pub fn unmarshalling(bytes: &Vec<u8>, offset: &mut usize) -> Result<Outpoint, &'static str> {
        if bytes.len() - *offset < 36 {
            return Err(
                "Los bytes recibidos no corresponden a un Outpoint, el largo es menor a 36 bytes",
            );
        }
        let mut tx_id: [u8; 32] = [0; 32];
        tx_id.copy_from_slice(&bytes[*offset..(*offset + 32)]);
        *offset += 32;
        let mut index_bytes: [u8; 4] = [0; 4];
        index_bytes.copy_from_slice(&bytes[*offset..(*offset + 4)]);
        *offset += 4;
        let index = u32::from_le_bytes(index_bytes);
        Ok(Outpoint { tx_id, index })
    }

    // Serializa el Outpoint según el protocolo bitcoin.
    // Se guarda en el array recibido.
    pub fn marshalling(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.tx_id[0..32]); // se cargan los elementos del tx_id
        let index_bytes: [u8; 4] = self.index.to_le_bytes();
        for item in index_bytes {
            bytes.push(item);
        }
    }

    /// Compara el hash recibido con el del outpoint.
    /// Devuelve true o false dependiendo de si coinciden o no,
    pub fn same_hash(&self, hash: [u8; 32]) -> bool {
        self.tx_id == hash
    }

    ///Devuelve el indice del outpoint
    pub fn index(&self) -> usize {
        self.index as usize
    }

    ///Devuelve el hash del outpoint
    pub fn hash(&self) -> [u8; 32] {
        self.tx_id
    }
}

#[cfg(test)]

mod test {
    use super::Outpoint;

    #[test]
    fn test_unmarshalling_del_outpoint_produce_tx_id_esperado() -> Result<(), &'static str> {
        let bytes: Vec<u8> = vec![1; 36];
        let tx_id_esperado: [u8; 32] = [1; 32];
        let mut offset: usize = 0;
        let outpoint: Outpoint = Outpoint::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(outpoint.tx_id, tx_id_esperado);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_del_outpoint_produce_index_esperado() -> Result<(), &'static str> {
        let mut bytes: Vec<u8> = vec![0; 36];
        for x in 0..4 {
            bytes[32 + x] = x as u8;
        }
        let index_esperado: u32 = 0x03020100;
        let mut offset: usize = 0;
        let outpoint: Outpoint = Outpoint::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(outpoint.index, index_esperado);
        Ok(())
    }

    #[test]
    fn test_marshalling_del_outpoint_produce_tx_id_esperado() -> Result<(), &'static str> {
        let mut marshalling_outpoint: Vec<u8> = Vec::new();
        let tx_id: [u8; 32] = [2; 32];
        let outpoint_to_marshalling: Outpoint = Outpoint {
            tx_id,
            index: 0x03020100,
        };
        outpoint_to_marshalling.marshalling(&mut marshalling_outpoint);
        let mut offset: usize = 0;
        let outpoint_unmarshaled: Outpoint =
            Outpoint::unmarshalling(&marshalling_outpoint, &mut offset)?;
        assert_eq!(outpoint_unmarshaled.tx_id, tx_id);
        Ok(())
    }

    #[test]
    fn test_marshalling_del_outpoint_produce_index_esperado() -> Result<(), &'static str> {
        let mut marshalling_outpoint: Vec<u8> = Vec::new();
        let tx_id: [u8; 32] = [2; 32];
        let index: u32 = 0x03020100;
        let outpoint_to_marshalling: Outpoint = Outpoint { tx_id, index };
        outpoint_to_marshalling.marshalling(&mut marshalling_outpoint);
        let mut offset: usize = 0;
        let outpoint_unmarshaled: Outpoint =
            Outpoint::unmarshalling(&marshalling_outpoint, &mut offset)?;
        assert_eq!(outpoint_unmarshaled.index, index);
        Ok(())
    }

    #[test]
    fn test_outpoint_correspondiente_a_una_coinbase_con_tx_id_nulo_devuelve_true() {
        let coinbase_outpoint: Outpoint = Outpoint::new([1; 32], 0xffffffff);
        assert!(coinbase_outpoint.is_a_coinbase_outpoint())
    }

    #[test]
    fn test_outpoint_correspondiente_a_una_coinbase_con_index_0xffffffff_devuelve_true() {
        let coinbase_outpoint: Outpoint = Outpoint::new([1; 32], 0xffffffff);
        assert!(coinbase_outpoint.is_a_coinbase_outpoint())
    }
}
