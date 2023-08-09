#[derive(Clone, Debug, PartialEq)]
/// Representa un entero de largo variable según se utiliza en el protocolo bitcoin.
pub struct CompactSizeUint {
    bytes: Vec<u8>,
}

impl CompactSizeUint {
    /// Crea el CompactSize según el número recibido
    pub fn new(value: u128) -> Self {
        CompactSizeUint {
            bytes: Self::generate_compact_size_uint(value),
        }
    }
    /// Genera los bytes del compact size segpun el número recibido.
    fn generate_compact_size_uint(value: u128) -> Vec<u8> {
        if (253..=0xffff).contains(&value) {
            return Self::get_compact_size_uint(0xfd, 3, value);
        }
        if (0x10000..=0xffffffff).contains(&value) {
            return Self::get_compact_size_uint(0xfe, 5, value);
        }
        if (0x100000000..=0xffffffffffffffff).contains(&value) {
            return Self::get_compact_size_uint(0xff, 9, value);
        }
        vec![value as u8]
    }

    /// Devuelve el CompactSize en formato bytes
    pub fn value(&self) -> &Vec<u8> {
        &self.bytes
    }
    /// Devuelve el CompactSize decodificado en formato u64
    pub fn decoded_value(&self) -> u64 {
        let mut bytes: [u8; 8] = [0; 8];
        bytes[0] = self.bytes[0];
        if bytes[0] == 0xfd {
            bytes[..2].copy_from_slice(&self.bytes[1..(2 + 1)]);
            return u64::from_le_bytes(bytes);
        }
        if bytes[0] == 0xfe {
            bytes[..4].copy_from_slice(&self.bytes[1..(4 + 1)]);
            return u64::from_le_bytes(bytes);
        }
        if bytes[0] == 0xff {
            bytes[..8].copy_from_slice(&self.bytes[1..(8 + 1)]);
            return u64::from_le_bytes(bytes);
        }
        u64::from_le_bytes(bytes)
    }

    /// Arma la cadena de bytes del compact size, según los parámetros recibidos
    fn get_compact_size_uint(first_byte: u8, bytes_amount: u8, value: u128) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();
        bytes.push(first_byte);
        let aux_bytes: [u8; 16] = value.to_le_bytes();
        let mut amount: u8 = 1;
        for byte in aux_bytes {
            if amount == bytes_amount {
                break;
            }
            bytes.push(byte);
            amount += 1;
        }
        bytes
    }

    /// Serializa el CompactSize a bytes y los devuelve
    pub fn marshalling(&self) -> Vec<u8> {
        let mut bytes = vec![];
        bytes.extend(self.value());
        bytes
    }

    /// Deserializa el CompactSize según los bytes recibidos y lo devuelve.
    /// Actualiza el offset.
    pub fn unmarshalling(
        bytes: &[u8],
        offset: &mut usize,
    ) -> Result<CompactSizeUint, &'static str> {
        if bytes.len() - (*offset) < 1 {
            return Err(
                "Los bytes recibidos no corresponden a un CompactSizeUnit, el largo es menor a 1 byte",
            );
        }
        let first_byte = bytes[*offset];
        *offset += 1;
        let mut value: Vec<u8> = Vec::new();
        value.push(first_byte);
        if first_byte == 0xfd {
            value.extend_from_slice(&bytes[*offset..(*offset + 2)]);
            *offset += 2;
            return Ok(Self { bytes: value });
        }
        if first_byte == 0xfe {
            value.extend_from_slice(&bytes[*offset..(*offset + 4)]);
            *offset += 4;
            return Ok(Self { bytes: value });
        }
        if first_byte == 0xff {
            value.extend_from_slice(&bytes[*offset..(*offset + 8)]);
            *offset += 8;
            return Ok(Self { bytes: value });
        }
        Ok(Self { bytes: value })
    }
}

#[cfg(test)]
mod test {
    use crate::compact_size_uint::CompactSizeUint;

    #[test]
    fn test_el_numero_200_se_representa_como_0x_c8() {
        let valor: u128 = 200;
        let valor_retornado: CompactSizeUint = CompactSizeUint::new(valor);
        let valor_esperado: Vec<u8> = vec![0xC8];
        assert_eq!(*valor_retornado.value(), valor_esperado);
    }

    #[test]
    fn test_el_numero_505_se_representa_como_0x_fd_f9_01() {
        let valor: u128 = 505;
        let valor_retornado: CompactSizeUint = CompactSizeUint::new(valor);
        let valor_esperado: Vec<u8> = vec![0xFD, 0xF9, 0x01];
        assert_eq!(*valor_retornado.value(), valor_esperado);
    }

    #[test]
    fn test_el_numero_100000_se_representa_como_0x_fe_a0_86_01_00() {
        let valor: u128 = 100000;
        let valor_retornado: CompactSizeUint = CompactSizeUint::new(valor);
        let valor_esperado: Vec<u8> = vec![0xFE, 0xA0, 0x86, 0x01, 0x00];
        assert_eq!(*valor_retornado.value(), valor_esperado);
    }

    #[test]
    fn test_el_numero_5000000000_se_representa_como_0x_ff_00_f2_05_2a_01_00_00_00() {
        let valor: u128 = 5000000000;
        let valor_retornado: CompactSizeUint = CompactSizeUint::new(valor);
        let valor_esperado: Vec<u8> = vec![0xFF, 0x00, 0xF2, 0x05, 0x2A, 0x01, 0x00, 0x00, 0x00];
        assert_eq!(*valor_retornado.value(), valor_esperado);
    }

    #[test]
    fn test_unmarshalling_de_compact_size_de_1_byte_se_realiza_correctamente(
    ) -> Result<(), &'static str> {
        let compact_size_serializado: Vec<u8> = vec![0x30];
        let mut offset: usize = 0;
        let compact_size_esperado: CompactSizeUint =
            CompactSizeUint::unmarshalling(&compact_size_serializado, &mut offset)?;
        assert_eq!(compact_size_esperado.bytes, compact_size_serializado);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_de_compact_size_de_3_bytes_se_realiza_correctamente(
    ) -> Result<(), &'static str> {
        let compact_size_serializado: Vec<u8> = vec![0xfd, 0x30, 0x20];
        let mut offset: usize = 0;
        let compact_size_esperado: CompactSizeUint =
            CompactSizeUint::unmarshalling(&compact_size_serializado, &mut offset)?;
        assert_eq!(compact_size_esperado.bytes, compact_size_serializado);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_de_compact_size_de_5_bytes_se_realiza_correctamente(
    ) -> Result<(), &'static str> {
        let compact_size_serializado: Vec<u8> = vec![0xFE, 0xA0, 0x86, 0x01, 0x00];
        let mut offset: usize = 0;
        let compact_size_esperado: CompactSizeUint =
            CompactSizeUint::unmarshalling(&compact_size_serializado, &mut offset)?;
        assert_eq!(compact_size_esperado.bytes, compact_size_serializado);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_de_compact_size_de_9_bytes_se_realiza_correctamente(
    ) -> Result<(), &'static str> {
        let compact_size_serializado: Vec<u8> =
            vec![0xFF, 0x00, 0xF2, 0x05, 0x2A, 0x01, 0x00, 0x00, 0x00];
        let mut offset: usize = 0;
        let compact_size_esperado: CompactSizeUint =
            CompactSizeUint::unmarshalling(&compact_size_serializado, &mut offset)?;
        assert_eq!(compact_size_esperado.bytes, compact_size_serializado);
        Ok(())
    }

    #[test]
    fn test_compact_size_de_valor_0x_fd_f9_01_devuelve_505_al_decodificarse() {
        let bytes: Vec<u8> = vec![0xfd, 0xf9, 0x01];
        let compact_size: CompactSizeUint = CompactSizeUint { bytes };
        let valor_esperado: u64 = compact_size.decoded_value();
        assert_eq!(valor_esperado, 505);
    }

    #[test]
    fn test_compact_size_de_valor_0x_c8_devuelve_200_al_decodificarse() {
        let bytes: Vec<u8> = vec![0xc8];
        let compact_size: CompactSizeUint = CompactSizeUint { bytes };
        let valor_esperado: u64 = compact_size.decoded_value();
        assert_eq!(valor_esperado, 200);
    }

    #[test]
    fn test_compact_size_de_valor_0x_fe_a0_86_01_00_devuelve_100000_al_decodificarse() {
        let bytes: Vec<u8> = vec![0xFE, 0xA0, 0x86, 0x01, 0x00];
        let compact_size: CompactSizeUint = CompactSizeUint { bytes };
        let valor_esperado: u64 = compact_size.decoded_value();
        assert_eq!(valor_esperado, 100000);
    }

    #[test]
    fn test_compact_size_de_valor_0x_ff_00_f2_05_2a_01_00_00_00_devuelve_5000000000_al_decodificarse(
    ) {
        let bytes: Vec<u8> = vec![0xFF, 0x00, 0xF2, 0x05, 0x2A, 0x01, 0x00, 0x00, 0x00];
        let compact_size: CompactSizeUint = CompactSizeUint { bytes };
        let valor_esperado: u64 = compact_size.decoded_value();
        assert_eq!(valor_esperado, 5000000000);
    }
}
