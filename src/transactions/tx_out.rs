use std::sync::{Arc, RwLock};

use gtk::glib;

use crate::{
    account::Account,
    compact_size_uint::CompactSizeUint,
    custom_errors::NodeCustomErrors,
    gtk::ui_events::{send_event_to_ui, UIEvent},
    logwriter::log_writer::{write_in_log, LogSender},
};

use super::{script::pubkey::Pubkey, transaction::Transaction};
/// Representa la estructura TxOut del protocolo bitcoin
#[derive(Debug, PartialEq, Clone)]
pub struct TxOut {
    value: i64,                       // Number of satoshis to spend
    pk_script_bytes: CompactSizeUint, // de 1 a 10.000 bytes
    pk_script: Pubkey, // Defines the conditions which must be satisfied to spend this output.
}

impl TxOut {
    /// Inicializa el TxOut según los parámetros recibidos.
    pub fn new(value: i64, pk_script_bytes: CompactSizeUint, pk_script: Vec<u8>) -> Self {
        TxOut {
            value,
            pk_script_bytes,
            pk_script: Pubkey::new(pk_script),
        }
    }
    /// Recibe una cadena de bytes correspondiente a un TxOut
    /// Devuelve un struct TxOut
    pub fn unmarshalling(bytes: &Vec<u8>, offset: &mut usize) -> Result<TxOut, &'static str> {
        if bytes.len() - (*offset) < 9 {
            return Err(
                "Los bytes recibidos no corresponden a un TxOut, el largo es menor a 9 bytes",
            );
        }
        let mut byte_value: [u8; 8] = [0; 8];
        byte_value.copy_from_slice(&bytes[*offset..*offset + 8]);
        *offset += 8;
        let value = i64::from_le_bytes(byte_value);
        let pk_script_bytes = CompactSizeUint::unmarshalling(bytes, offset)?;
        let mut pk_script: Vec<u8> = Vec::new();
        let amount_bytes: usize = pk_script_bytes.decoded_value() as usize;
        pk_script.extend_from_slice(&bytes[*offset..(*offset + amount_bytes)]);
        *offset += amount_bytes;
        Ok(TxOut {
            value,
            pk_script_bytes,
            pk_script: Pubkey::new(pk_script),
        })
    }

    /// Recibe un vector de bytes que contiene los txout y un offset indicando la posicion donde empiezan.
    /// Devuelve un vector de txout completando los campos según los bytes leidos en caso de que todo este bien
    /// y un string indicando el error cuando algo falla. Actualiza el offset
    pub fn unmarshalling_txouts(
        bytes: &Vec<u8>,
        amount_txout: u64,
        offset: &mut usize,
    ) -> Result<Vec<TxOut>, &'static str> {
        let mut tx_out_list: Vec<TxOut> = Vec::new();
        let mut i = 0;
        while i < amount_txout {
            tx_out_list.push(Self::unmarshalling(bytes, offset)?);
            i += 1;
        }
        Ok(tx_out_list)
    }

    /// Serializa el TxOut a bytes según el protocolo bitcoin.
    /// Los guarda en el vector recibido por parámetro.
    pub fn marshalling(&self, bytes: &mut Vec<u8>) {
        let value_bytes = self.value.to_le_bytes();
        bytes.extend_from_slice(&value_bytes[0..8]);
        let pk_script_bytes: Vec<u8> = self.pk_script_bytes.marshalling();
        bytes.extend_from_slice(&pk_script_bytes[0..pk_script_bytes.len()]);
        bytes.extend_from_slice(self.pk_script.bytes());
    }

    /// Devuelve el valor del TxOut
    pub fn value(&self) -> i64 {
        self.value
    }

    /// Obtiene la address del receptor del TxOut
    pub fn get_address(&self) -> Result<String, &'static str> {
        self.pk_script.generate_address()
    }
    /// Devuelve el pub key script
    pub fn get_pub_key_script(&self) -> &Vec<u8> {
        self.pk_script.bytes()
    }

    /// Recibe un puntero a un puntero que apunta a las cuentas de la wallet y una transaccion y se fija si el address de la tx_out
    /// es igual a algun address de la wallet. Si encunetra una coincidencia agrega la transaccion al vector de pending_transactions de la cuenta. En caso exitoso
    /// devuelve Ok(()) y en caso de algun error devuevle el error especifico
    pub fn involves_user_account(
        &self,
        log_sender: &LogSender,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        accounts: Arc<RwLock<Arc<RwLock<Vec<Account>>>>>,
        tx: Transaction,
    ) -> Result<(), NodeCustomErrors> {
        for account in &*accounts
            .read()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .read()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
        {
            if !account
                .pending_transactions
                .read()
                .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                .contains(&tx)
            {
                let tx_asociate_address = match self.get_address() {
                    Ok(address) => address,
                    Err(e) => e.to_string(),
                };
                if tx_asociate_address == account.address {
                    write_in_log(
                        &log_sender.info_log_sender,
                        format!(
                            "Transaccion pendiente {:?} -- involucra a la cuenta {:?}",
                            tx.hex_hash(),
                            account.address
                        )
                        .as_str(),
                    );
                    println!("\nTRANSACCION: {} \nINVOLUCRA A LA CUENTA: {}\nAUN NO SE ENCUENTRA EN UN BLOQUE (PENDIENTE)", tx.hex_hash(), account.address);
                    send_event_to_ui(
                        ui_sender,
                        UIEvent::ShowPendingTransaction(account.clone(), tx.clone()),
                    );
                    account
                        .pending_transactions
                        .write()
                        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                        .push(tx.clone());
                }
            }
        }
        Ok(())
    }

    /// Devuelve true o false dependiendo de si la transaccion fue enviada a la cuenta recibida por parametro
    pub fn is_sent_to_account(&self, address: &String) -> Result<bool, &'static str> {
        let tx_asociate_address = self.get_address()?;
        if tx_asociate_address.eq(address) {
            return Ok(true);
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use crate::compact_size_uint::CompactSizeUint;

    use super::TxOut;

    fn simular_flujo_de_datos(value: i64, compact_size_value: u128) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();
        let compact_size: CompactSizeUint = CompactSizeUint::new(compact_size_value);
        let mut pk_script: Vec<u8> = Vec::new();
        for _x in 0..compact_size_value {
            pk_script.push(1);
        }
        let tx_out: TxOut = TxOut::new(value, compact_size, pk_script);
        tx_out.marshalling(&mut bytes);
        bytes
    }

    #[test]
    fn test_unmarshalling_tx_out_invalido() {
        let bytes: Vec<u8> = vec![0; 3];
        let mut offset: usize = 0;
        let tx_out = TxOut::unmarshalling(&bytes, &mut offset);
        assert!(tx_out.is_err());
    }

    #[test]
    fn test_unmarshalling_tx_out_con_value_valido_y_0_pkscript() -> Result<(), &'static str> {
        let bytes: Vec<u8> = vec![0; 9];
        let mut offset: usize = 0;
        let tx_out = TxOut::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(tx_out.value, 0);
        assert_eq!(tx_out.pk_script_bytes.decoded_value(), 0);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_tx_out_con_value_valido_y_1_pkscript() -> Result<(), &'static str> {
        let mut bytes: Vec<u8> = vec![0; 8];
        bytes[0] = 1; //Está en little endian
        let pk_script_compact_size = CompactSizeUint::new(1);
        bytes.extend_from_slice(pk_script_compact_size.value());
        let pk_script: [u8; 1] = [10; 1];
        bytes.extend_from_slice(&pk_script);
        let mut offset: usize = 0;
        let tx_out = TxOut::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(tx_out.value, 1);
        assert_eq!(
            tx_out.pk_script_bytes.decoded_value(),
            pk_script_compact_size.decoded_value()
        );
        assert_eq!(tx_out.pk_script.bytes()[0], pk_script[0]);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_con_2_tx_out_devuelve_offset_esperado() -> Result<(), &'static str> {
        let bytes: Vec<u8> = vec![0; 18];
        let mut offset: usize = 0;
        let _tx_out = TxOut::unmarshalling_txouts(&bytes, 2, &mut offset)?;
        assert_eq!(offset, 18);
        Ok(())
    }
    #[test]
    fn test_unmarshalling_con_menos_bytes_de_los_esperados_devuelve_error(
    ) -> Result<(), &'static str> {
        let bytes: Vec<u8> = vec![0; 14];
        let mut offset: usize = 0;
        let tx_out: Result<Vec<TxOut>, &'static str> =
            TxOut::unmarshalling_txouts(&bytes, 2, &mut offset);
        assert!(tx_out.is_err());
        Ok(())
    }

    #[test]
    fn test_marshalling_de_tx_out_devuelve_value_esperado() -> Result<(), &'static str> {
        let expected_value: i64 = 0x302010;
        let bytes: Vec<u8> = simular_flujo_de_datos(expected_value, 3);
        let mut offset: usize = 0;
        let tx_out_expected: TxOut = TxOut::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(tx_out_expected.value, expected_value);
        Ok(())
    }

    #[test]
    fn test_marshalling_de_tx_out_devuelve_pk_script_bytes_esperado() -> Result<(), &'static str> {
        let compact_size_value: u128 = 43;
        let value: i64 = 0x302010;
        let bytes: Vec<u8> = simular_flujo_de_datos(value, compact_size_value);
        let mut offset: usize = 0;
        let tx_out_expected: TxOut = TxOut::unmarshalling(&bytes, &mut offset)?;
        let compact_size_expected: CompactSizeUint = CompactSizeUint::new(compact_size_value);
        assert_eq!(tx_out_expected.pk_script_bytes, compact_size_expected);
        Ok(())
    }
    #[test]
    fn test_marshalling_de_tx_out_devuelve_pk_script_esperado() -> Result<(), &'static str> {
        let compact_size_value: u128 = 4;
        let value: i64 = 0x302010;
        let bytes: Vec<u8> = simular_flujo_de_datos(value, compact_size_value);
        let mut offset: usize = 0;
        let tx_out_expected: TxOut = TxOut::unmarshalling(&bytes, &mut offset)?;
        let pk_script_expected: Vec<u8> = vec![1, 1, 1, 1];
        assert_eq!(*tx_out_expected.pk_script.bytes(), pk_script_expected);
        Ok(())
    }
}
