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
#[derive(Debug, PartialEq, Clone)]
/// Represents the TxOut structure of the bitcoin protocol
pub struct TxOut {
    value: i64,                       // Number of satoshis to spend
    pk_script_bytes: CompactSizeUint, // from 1 to 10.000 bytes
    pk_script: Pubkey, // Defines the conditions which must be satisfied to spend this output.
}

impl TxOut {
    /// Initializes the TxOut according to the received parameters.
    pub fn new(value: i64, pk_script_bytes: CompactSizeUint, pk_script: Vec<u8>) -> Self {
        TxOut {
            value,
            pk_script_bytes,
            pk_script: Pubkey::new(pk_script),
        }
    }
    /// Receives a vector of bytes corresponding to a TxOut.
    /// Returns a TxOut struct
    pub fn unmarshalling(bytes: &Vec<u8>, offset: &mut usize) -> Result<TxOut, &'static str> {
        if bytes.len() - (*offset) < 9 {
            return Err(
                "The bytes vector is not long enough to unmarshall a TxOut. It must be at least 9 bytes long",
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

    /// Receives a vector of bytes containing the txouts and an offset indicating the position where they begin.
    /// Returns a vector of txouts completing the fields according to the bytes read in case everything is fine
    /// and a string indicating the error when something fails. Updates the offset.
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

    /// Marshalls the TxOut to bytes according to the bitcoin protocol.
    /// Saves them in the vector received by parameter.
    pub fn marshalling(&self, bytes: &mut Vec<u8>) {
        let value_bytes = self.value.to_le_bytes();
        bytes.extend_from_slice(&value_bytes[0..8]);
        let pk_script_bytes: Vec<u8> = self.pk_script_bytes.marshalling();
        bytes.extend_from_slice(&pk_script_bytes[0..pk_script_bytes.len()]);
        bytes.extend_from_slice(self.pk_script.bytes());
    }

    /// Returns the value of the TxOut
    pub fn value(&self) -> i64 {
        self.value
    }

    /// Gets the address of the receiver of the TxOut
    pub fn get_address(&self) -> Result<String, &'static str> {
        self.pk_script.generate_address()
    }
    /// Returns the pub key script
    pub fn get_pub_key_script(&self) -> &Vec<u8> {
        self.pk_script.bytes()
    }

    /// Recibe un puntero a un puntero que apunta a las cuentas de la wallet y una transaccion y se fija si el address de la tx_out
    /// es igual a algun address de la wallet. Si encunetra una coincidencia agrega la transaccion al vector de pending_transactions de la cuenta. En caso exitoso
    /// devuelve Ok(()) y en caso de algun error devuevle el error especifico
    /// Receives a pointer to a pointer that points to the accounts of the wallet and a transaction and checks if the address of the tx_out
    /// is equal to some address of the wallet. If it finds a match, it adds the transaction to the pending_transactions vector of the account. In case of success
    /// returns Ok(()) and in case of any error it returns the specific error.
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
                            "Pending transaction {:?} -- involves the account {:?}",
                            tx.hex_hash(),
                            account.address
                        )
                        .as_str(),
                    );
                    println!("TRANSACTION: {} \nINVOLVES THE ACCOUNT: {}\nSTILL NOT CONFIRMED IN A BLOCK (PENDING)", tx.hex_hash(), account.address);
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

    /// Returns true or false depending on whether the transaction was sent to the account received by parameter.
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

    fn simulate_data_flow(value: i64, compact_size_value: u128) -> Vec<u8> {
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
    fn test_unmarshalling_invalid_tx_out() {
        let bytes: Vec<u8> = vec![0; 3];
        let mut offset: usize = 0;
        let tx_out = TxOut::unmarshalling(&bytes, &mut offset);
        assert!(tx_out.is_err());
    }

    #[test]
    fn test_unmarshalling_tx_out_with_valid_value_and_empty_pk_script() -> Result<(), &'static str> {
        let bytes: Vec<u8> = vec![0; 9];
        let mut offset: usize = 0;
        let tx_out = TxOut::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(tx_out.value, 0);
        assert_eq!(tx_out.pk_script_bytes.decoded_value(), 0);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_tx_out_with_valid_value_and_nonempty_pk_script() -> Result<(), &'static str> {
        let mut bytes: Vec<u8> = vec![0; 8];
        bytes[0] = 1; // In little endian
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
    fn test_unmarshalling_with_2_tx_outs_returns_expected_offset() -> Result<(), &'static str> {
        let bytes: Vec<u8> = vec![0; 18];
        let mut offset: usize = 0;
        let _tx_out = TxOut::unmarshalling_txouts(&bytes, 2, &mut offset)?;
        assert_eq!(offset, 18);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_with_less_bytes_than_expected_returns_error() -> Result<(), &'static str> {
        let bytes: Vec<u8> = vec![0; 14];
        let mut offset: usize = 0;
        let tx_out: Result<Vec<TxOut>, &'static str> =
            TxOut::unmarshalling_txouts(&bytes, 2, &mut offset);
        assert!(tx_out.is_err());
        Ok(())
    }

    #[test]
    fn test_tx_out_marshalling_returns_expected_value() -> Result<(), &'static str> {
        let expected_value: i64 = 0x302010;
        let bytes: Vec<u8> = simulate_data_flow(expected_value, 3);
        let mut offset: usize = 0;
        let tx_out_expected: TxOut = TxOut::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(tx_out_expected.value, expected_value);
        Ok(())
    }

    #[test]
    fn test_tx_out_marshalling_returns_expected_pk_script_bytes() -> Result<(), &'static str> {
        let compact_size_value: u128 = 43;
        let value: i64 = 0x302010;
        let bytes: Vec<u8> = simulate_data_flow(value, compact_size_value);
        let mut offset: usize = 0;
        let tx_out_expected: TxOut = TxOut::unmarshalling(&bytes, &mut offset)?;
        let compact_size_expected: CompactSizeUint = CompactSizeUint::new(compact_size_value);
        assert_eq!(tx_out_expected.pk_script_bytes, compact_size_expected);
        Ok(())
    }

    #[test]
    fn test_tx_out_marshalling_returns_expected_pk_script() -> Result<(), &'static str> {
        let compact_size_value: u128 = 4;
        let value: i64 = 0x302010;
        let bytes: Vec<u8> = simulate_data_flow(value, compact_size_value);
        let mut offset: usize = 0;
        let tx_out_expected: TxOut = TxOut::unmarshalling(&bytes, &mut offset)?;
        let pk_script_expected: Vec<u8> = vec![1, 1, 1, 1];
        assert_eq!(*tx_out_expected.pk_script.bytes(), pk_script_expected);
        Ok(())
    }
}
