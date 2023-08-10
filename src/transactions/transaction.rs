use std::{
    collections::HashMap,
    error::Error,
    io,
    sync::{Arc, RwLock},
};

use bitcoin_hashes::{sha256, sha256d, Hash};
use gtk::glib;

use crate::{
    account::Account, compact_size_uint::CompactSizeUint, custom_errors::NodeCustomErrors,
    gtk::ui_events::UIEvent, logwriter::log_writer::LogSender, utxo_tuple::UtxoTuple,
};

use super::{
    outpoint::Outpoint,
    script::{
        p2pkh_script::{self, generate_pubkey_script},
        sig_script::SigScript,
    },
    tx_in::TxIn,
    tx_out::TxOut,
};

const SIG_HASH_ALL: u32 = 0x00000001;
const TRANSACTION_VERSION: i32 = 0x00000002;

#[derive(Debug, PartialEq, Clone)]
/// Represents a bitcoin transaction
pub struct Transaction {
    pub version: i32,
    pub txin_count: CompactSizeUint,
    pub tx_in: Vec<TxIn>,
    pub txout_count: CompactSizeUint,
    pub tx_out: Vec<TxOut>,
    pub lock_time: u32,
}

impl Transaction {
    /// Creates the transaction with the received parameters.
    pub fn new(
        version: i32,
        txin_count: CompactSizeUint,
        tx_in: Vec<TxIn>,
        txout_count: CompactSizeUint,
        tx_out: Vec<TxOut>,
        lock_time: u32,
    ) -> Self {
        Transaction {
            version,
            txin_count,
            tx_in,
            txout_count,
            tx_out,
            lock_time,
        }
    }

    /// Unmarshalls the transaction from a byte array.
    /// Returns the transaction or an error if the byte array doesn't comply with the format.
    pub fn unmarshalling(bytes: &Vec<u8>, offset: &mut usize) -> Result<Transaction, &'static str> {
        // en teoria se lee el coinbase transaccion primero
        if bytes.len() < 10 {
            return Err(
                "The byte array is too short to be a transaction. It must be at least 10 bytes.",
            );
        }
        let mut version_bytes: [u8; 4] = [0; 4];
        version_bytes.copy_from_slice(&bytes[*offset..(*offset + 4)]);
        *offset += 4;
        let version = i32::from_le_bytes(version_bytes);
        let txin_count: CompactSizeUint = CompactSizeUint::unmarshalling(bytes, &mut *offset)?;
        let amount_txin: u64 = txin_count.decoded_value();
        let tx_in: Vec<TxIn> = TxIn::unmarshalling_txins(bytes, amount_txin, &mut *offset)?; // update offset
        if tx_in[0].is_coinbase() && txin_count.decoded_value() != 1 {
            return Err("A coinbase transaction must have only one txin.");
        }
        let txout_count: CompactSizeUint = CompactSizeUint::unmarshalling(bytes, &mut *offset)?;
        let amount_txout: u64 = txout_count.decoded_value();
        let tx_out: Vec<TxOut> = TxOut::unmarshalling_txouts(bytes, amount_txout, &mut *offset)?; // update offset
        let mut lock_time_bytes: [u8; 4] = [0; 4];
        lock_time_bytes.copy_from_slice(&bytes[*offset..(*offset + 4)]);
        *offset += 4;
        let lock_time = u32::from_le_bytes(lock_time_bytes);
        Ok(Transaction {
            version,
            txin_count,
            tx_in,
            txout_count,
            tx_out,
            lock_time,
        })
    }

    /// Marshalls the transaction.
    /// Stores the bytes in the reference of the received vector.
    pub fn marshalling(&self, bytes: &mut Vec<u8>) {
        let version_bytes: [u8; 4] = self.version.to_le_bytes();
        bytes.extend_from_slice(&version_bytes);
        bytes.extend_from_slice(&self.txin_count.marshalling());
        for tx_in in &self.tx_in {
            tx_in.marshalling(bytes);
        }
        bytes.extend_from_slice(&self.txout_count.marshalling());
        for tx_out in &self.tx_out {
            tx_out.marshalling(bytes);
        }
        let locktime_bytes: [u8; 4] = self.lock_time.to_le_bytes();
        bytes.extend_from_slice(&locktime_bytes);
    }
    /// Returs the hash of the transaction
    pub fn hash(&self) -> [u8; 32] {
        self.hash_message(false)
    }
    /// Hashes the transaction.
    /// If it receives true, it pushes the bytes corresponding to the SIGHASH_ALL inside the vector.
    /// Otherwise, it hashes normally.
    fn hash_message(&self, is_message: bool) -> [u8; 32] {
        let mut raw_transaction_bytes: Vec<u8> = Vec::new();
        self.marshalling(&mut raw_transaction_bytes);
        if is_message {
            let bytes = SIG_HASH_ALL.to_le_bytes();
            raw_transaction_bytes.extend_from_slice(&bytes);
        }
        if is_message {
            let hash_transaction = sha256::Hash::hash(&raw_transaction_bytes);
            return *hash_transaction.as_byte_array();
        }
        let hash_transaction = sha256d::Hash::hash(&raw_transaction_bytes);
        *hash_transaction.as_byte_array()
    }

    /// Receives a reference to a vector of bytes and the amount of transactions to deserialize.
    /// Returns a vector with the transactions or an error. Updates the offset.
    pub fn unmarshalling_transactions(
        bytes: &Vec<u8>,
        amount_transactions: u64,
        offset: &mut usize,
    ) -> Result<Vec<Transaction>, &'static str> {
        let mut transactions_list: Vec<Transaction> = Vec::new();
        let mut i = 0;
        while i < amount_transactions {
            transactions_list.push(Self::unmarshalling(bytes, offset)?);
            i += 1;
        }
        Ok(transactions_list)
    }

    /// Returns true or false depending if the transaction is a coinbase
    pub fn is_coinbase_transaction(&self) -> bool {
        self.tx_in[0].is_coinbase()
    }

    /// Returns a copy of the tx_out of the transaction
    pub fn get_txout(&self) -> Vec<TxOut> {
        self.tx_out.clone()
    }

    /// Checks the inputs of the transaction and removes the utxos that were spent
    pub fn remove_utxos(
        &self,
        utxo_set: Arc<RwLock<HashMap<[u8; 32], UtxoTuple>>>,
    ) -> Result<(), Box<dyn Error>> {
        // If the tx spends an existing output in our utxo_set, we remove it
        for txin in &self.tx_in {
            let txid = &txin.get_previous_output_hash();
            let output_index = txin.get_previous_output_index();
            if utxo_set
                .read()
                .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                .contains_key(txid)
            {
                if let Some(utxo) = utxo_set
                    .write()
                    .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                    .get_mut(txid)
                {
                    utxo.remove_utxo(output_index);
                }
            }
        }
        Ok(())
    }

    /// Generates the UtxoTuple and saves it in the utxo_set
    pub fn load_utxos(
        &self,
        utxo_set: Arc<RwLock<HashMap<[u8; 32], UtxoTuple>>>,
    ) -> Result<(), Box<dyn Error>> {
        let hash = self.hash();
        let mut utxos_and_index = Vec::new();
        for (position, utxo) in self.tx_out.iter().enumerate() {
            let utxo_and_index = (utxo.clone(), position);
            utxos_and_index.push(utxo_and_index);
        }
        let utxo_tuple = UtxoTuple::new(hash, utxos_and_index);
        utxo_set
            .write()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .insert(hash, utxo_tuple);
        Ok(())
    }

    /// Returns a string that represents the hash of the transaction in hexadecimal and in the format
    /// used in the page https://blockstream.info/testnet/ to show transactions
    pub fn hex_hash(&self) -> String {
        let hash_as_bytes = self.hash();
        let inverted_hash: [u8; 32] = {
            let mut inverted = [0; 32];
            for (i, byte) in hash_as_bytes.iter().enumerate() {
                inverted[31 - i] = *byte;
            }
            inverted
        };
        let hex_hash = inverted_hash
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect();
        hex_hash
    }

    /// Receives a pointer to a pointer with the accounts of the wallet and checks if any tx_out has an address
    /// equal to any of the wallet. Returns Ok(()) if no error occurs or specific Error otherwise.
    pub fn check_if_tx_involves_user_account(
        &self,
        log_sender: &LogSender,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        accounts: Arc<RwLock<Arc<RwLock<Vec<Account>>>>>,
    ) -> Result<(), NodeCustomErrors> {
        for tx_out in self.tx_out.clone() {
            tx_out.involves_user_account(log_sender, ui_sender, accounts.clone(), self.clone())?;
        }
        Ok(())
    }
    /// Generates the unsigned transaction, the parameters indicate the address
    /// where the amount (value) will be sent, the reward for adding the new transaction
    /// to the block (fee) and the address to return the change in case it is generated (change_address).
    pub fn generate_unsigned_transaction(
        address_receiver: &str,
        change_adress: &str,
        value: i64,
        fee: i64,
        utxos_to_spend: &Vec<UtxoTuple>,
    ) -> Result<Transaction, Box<dyn Error>> {
        let mut tx_ins: Vec<TxIn> = Vec::new();
        let mut input_balance: i64 = 0;
        // Generation of tx_in with the reference of the utxos. The satoshis to be spent are obtained from here.
        // ¡Attention! There can be more than one.
        for utxo in utxos_to_spend {
            let tx_id: [u8; 32] = utxo.hash();
            input_balance += utxo.balance();
            let indexes: Vec<usize> = utxo.get_indexes_from_utxos();
            for index in indexes {
                let previous_output: Outpoint = Outpoint::new(tx_id, index as u32);
                let tx_in: TxIn = TxIn::incomplete_txin(previous_output);
                tx_ins.push(tx_in);
            }
        }
        // Contains the amount corresponding to the change of the tx
        let change_amount: i64 = input_balance - (value + fee);
        // Amount of txIn created in the previous steps
        let txin_count: CompactSizeUint = CompactSizeUint::new(tx_ins.len() as u128);
        // Vec containing the outputs of our transaction
        let mut tx_outs: Vec<TxOut> = Vec::new();
        // Creation of the pubkey_script where we transfer the satoshis
        let target_pk_script: Vec<u8> = generate_pubkey_script(address_receiver)?;
        let target_pk_script_bytes: CompactSizeUint =
            CompactSizeUint::new(target_pk_script.len() as u128);
        // Creation of the txOut (utxo) referenced to the address that was sent to us.
        let utxo_to_send: TxOut = TxOut::new(value, target_pk_script_bytes, target_pk_script);
        tx_outs.push(utxo_to_send);
        // Creation of the pubkey_script where we will send the change of our tx.
        let change_pk_script: Vec<u8> = generate_pubkey_script(change_adress)?;
        let change_pk_script_bytes: CompactSizeUint =
            CompactSizeUint::new(change_pk_script.len() as u128);
        let change_utxo: TxOut =
            TxOut::new(change_amount, change_pk_script_bytes, change_pk_script);
        tx_outs.push(change_utxo);
        let txout_count = CompactSizeUint::new(tx_outs.len() as u128);
        // lock_time = 0 => Not locked
        let lock_time: u32 = 0;
        let incomplete_transaction = Transaction::new(
            TRANSACTION_VERSION,
            txin_count,
            tx_ins,
            txout_count,
            tx_outs,
            lock_time,
        );
        Ok(incomplete_transaction)
    }

    /// Signs the transaction.
    /// Receives the list of utxos to spend and adds the signature_script to each TxIn.
    pub fn sign(
        &mut self,
        account: &Account,
        utxos_to_spend: &Vec<UtxoTuple>,
    ) -> Result<(), Box<dyn Error>> {
        let mut signatures = Vec::new();
        for index in 0..self.tx_in.len() {
            // add signature to each input
            let z = self.generate_message_to_sign(index, utxos_to_spend);
            signatures.push(SigScript::generate_sig_script(z, account)?);
        }
        for (index, signature) in signatures.into_iter().enumerate() {
            self.tx_in[index].add(signature);
        }
        Ok(())
    }

    /// Generates the txin with the previous pubkey of the received tx_in.
    /// Returns the hash.
    fn generate_message_to_sign(
        &self,
        tx_in_index: usize,
        utxos_to_spend: &Vec<UtxoTuple>,
    ) -> [u8; 32] {
        let mut tx_copy = self.clone();
        let mut script = Vec::new();
        let input_to_sign = &tx_copy.tx_in[tx_in_index];
        for utxos in utxos_to_spend {
            let pubkey = utxos.find(
                input_to_sign.get_previous_output_hash(),
                input_to_sign.get_previous_output_index(),
            );
            script = match pubkey {
                Some(value) => value.to_vec(),
                None => continue,
            };
        }
        tx_copy.tx_in[tx_in_index].set_signature_script(script);
        tx_copy.hash_message(true)
    }

    /// Validates the transaction.
    /// Executes the script and returns an error if it does not pass the validation.
    pub fn validate(&self, utxos_to_spend: &Vec<UtxoTuple>) -> Result<(), Box<dyn Error>> {
        let mut p2pkh_scripts = Vec::new();
        for utxo in utxos_to_spend {
            for (txout, _) in &utxo.utxo_set {
                p2pkh_scripts.push(txout.get_pub_key_script())
            }
        }

        for (index, txin) in self.tx_in.iter().enumerate() {
            //txin.
            if !p2pkh_script::validate(p2pkh_scripts[index], txin.signature_script.get_bytes())? {
                return Err(Box::new(std::io::Error::new(
                    io::ErrorKind::Other,
                    "The p2pkh script is not valid",
                )));
            }
        }
        Ok(())
    }

    /// Returns the amount of the transaction.
    pub fn amount(&self) -> i64 {
        let mut amount = 0;
        for txout in &self.tx_out {
            amount += txout.value();
        }
        amount
    }
    /// Returns the height of the block in which the transaction is located.
    /// Valid only for coinbase transactions.
    pub fn get_height(&self) -> u32 {
        self.tx_in[0].get_height()
    }

    /// Returns the amount sent to addresses other than the one received by parameter.
    pub fn amount_spent_by_account(&self, address: &String) -> Result<i64, Box<dyn Error>> {
        let mut amount = 0;
        for txout in &self.tx_out {
            if !txout.is_sent_to_account(address)? {
                amount += txout.value();
            }
        }
        Ok(amount)
    }
}

#[cfg(test)]

mod test {
    use super::Transaction;
    use crate::{
        compact_size_uint::CompactSizeUint,
        transactions::script::sig_script::SigScript,
        transactions::{outpoint::Outpoint, tx_in::TxIn, tx_out::TxOut},
    };
    use bitcoin_hashes::{sha256d, Hash};

    /// Auxiliar function that creates the txin
    fn create_txin(amount: u128) -> Vec<TxIn> {
        let mut tx_in: Vec<TxIn> = Vec::new();
        for _i in 0..amount {
            let tx_id: [u8; 32] = [1; 32];
            let index_outpoint: u32 = 0x30000000;
            let outpoint: Outpoint = Outpoint::new(tx_id, index_outpoint);
            let compact_txin: CompactSizeUint = CompactSizeUint::new(1);
            let bytes: Vec<u8> = vec![1];
            let signature_script = SigScript::new(bytes);
            let sequence: u32 = 0xffffffff;
            tx_in.push(TxIn::new(
                outpoint,
                compact_txin,
                None,
                signature_script,
                sequence,
            ));
        }
        tx_in
    }

    /// Auxiliar function that creates the txout
    fn create_txout(amount: u128) -> Vec<TxOut> {
        let mut tx_out: Vec<TxOut> = Vec::new();
        for _i in 0..amount {
            let value: i64 = 43;
            let pk_script_bytes: CompactSizeUint = CompactSizeUint::new(0);
            let pk_script: Vec<u8> = Vec::new();
            tx_out.push(TxOut::new(value, pk_script_bytes, pk_script));
        }
        tx_out
    }

    /// Auxiliar function that creates the byte array to test the deserialization
    fn generate_data_stream(
        version: i32,
        tx_in_count: u128,
        tx_out_count: u128,
        lock_time: u32,
    ) -> Vec<u8> {
        // bytes container
        let mut bytes: Vec<u8> = Vec::new();
        // version settings
        let version: i32 = version;
        // tx_in_count settings
        let txin_count = CompactSizeUint::new(tx_in_count);
        // tx_in settings
        let tx_in: Vec<TxIn> = create_txin(tx_in_count);
        // tx_out_count settings
        let txout_count = CompactSizeUint::new(tx_out_count);
        // tx_out settings
        let tx_out: Vec<TxOut> = create_txout(tx_out_count);
        //lock_time settings
        let lock_time: u32 = lock_time;
        let transaction: Transaction =
            Transaction::new(version, txin_count, tx_in, txout_count, tx_out, lock_time);
        transaction.marshalling(&mut bytes);
        bytes
    }

    #[test]
    fn test_transaction_hashes_correctly() {
        let previous_output: Outpoint = Outpoint::new([1; 32], 0x11111111);
        let script_bytes: CompactSizeUint = CompactSizeUint::new(0);
        let mut tx_in: Vec<TxIn> = Vec::new();
        tx_in.push(TxIn::new(
            previous_output,
            script_bytes,
            None,
            SigScript::new(Vec::new()),
            0x11111111,
        ));
        let pk_script_bytes: CompactSizeUint = CompactSizeUint::new(0);
        let mut tx_out: Vec<TxOut> = Vec::new();
        tx_out.push(TxOut::new(0x1111111111111111, pk_script_bytes, Vec::new()));
        let txin_count: CompactSizeUint = CompactSizeUint::new(1);
        let txout_count: CompactSizeUint = CompactSizeUint::new(1);
        let transaction: Transaction = Transaction::new(
            0x11111111,
            txin_count,
            tx_in,
            txout_count,
            tx_out,
            0x11111111,
        );
        let mut vector = Vec::new();
        transaction.marshalling(&mut vector);
        let hash_transaction = sha256d::Hash::hash(&vector);
        assert_eq!(transaction.hash(), *hash_transaction.as_byte_array());
    }

    #[test]
    fn test_unmarshalling_invalid_transaction() {
        let bytes: Vec<u8> = vec![0; 5];
        let mut offset: usize = 0;
        let transaction = Transaction::unmarshalling(&bytes, &mut offset);
        assert!(transaction.is_err());
    }

    #[test]
    fn test_unmarshalling_transaction_with_coinbase_and_more_inputs_returns_error() {
        // Byte container
        let mut bytes: Vec<u8> = Vec::new();
        // Version settings
        let version: i32 = 23;
        let version_bytes = version.to_le_bytes();
        bytes.extend_from_slice(&version_bytes[0..4]);
        // Tx_in_count settings
        let txin_count = CompactSizeUint::new(2);
        bytes.extend_from_slice(&txin_count.marshalling()[0..1]);
        // Tx_in settings
        let tx_id: [u8; 32] = [0; 32];
        let index_outpoint: u32 = 0xffffffff;
        let outpoint: Outpoint = Outpoint::new(tx_id, index_outpoint);
        let compact_txin: CompactSizeUint = CompactSizeUint::new(5);
        let height = Some(vec![1, 1, 1, 1]);
        let bytes_to_sig: Vec<u8> = vec![1];
        let signature_script = SigScript::new(bytes_to_sig);
        let sequence: u32 = 0xffffffff;
        let mut tx_in: Vec<TxIn> = Vec::new();
        tx_in.push(TxIn::new(
            outpoint,
            compact_txin,
            height,
            signature_script,
            sequence,
        ));
        tx_in[0 as usize].marshalling(&mut bytes);
        let txin_amount: u128 = txin_count.decoded_value() as u128;
        let tx_input: Vec<TxIn> = create_txin(txin_amount);
        tx_input[0 as usize].marshalling(&mut bytes);
        // Tx_out_count settings
        let txout_count = CompactSizeUint::new(1);
        bytes.extend_from_slice(txout_count.value());
        // Tx_out settings
        let txout_amount: u128 = txout_count.decoded_value() as u128;
        let tx_out: Vec<TxOut> = create_txout(txout_amount);
        tx_out[0 as usize].marshalling(&mut bytes);
        // Lock_time settings
        let lock_time: [u8; 4] = [0; 4];
        bytes.extend_from_slice(&lock_time);

        let mut offset: usize = 0;
        let transaction: Result<Transaction, &'static str> =
            Transaction::unmarshalling(&bytes, &mut offset);
        assert!(transaction.is_err());
    }


    #[test]
    fn test_unmarshalling_transaction_returns_expected_version() -> Result<(), &'static str> {
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generate_data_stream(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(transaction.version, version);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_returns_expected_txin_count() -> Result<(), &'static str> {
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generate_data_stream(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        let tx_count_expected: CompactSizeUint = CompactSizeUint::new(tx_in_count);
        assert_eq!(transaction.txin_count, tx_count_expected);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_returns_expected_txin() -> Result<(), &'static str> {
        let tx_in_count: u128 = 1;
        let tx_out_count: u128 = 1;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generate_data_stream(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        let tx_in: Vec<TxIn> = create_txin(tx_in_count);
        assert_eq!(transaction.tx_in, tx_in);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_returns_expected_txout_count() -> Result<(), &'static str> {
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generate_data_stream(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        let tx_count_expected: CompactSizeUint = CompactSizeUint::new(tx_out_count);
        assert_eq!(transaction.txout_count, tx_count_expected);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_returns_expected_txout() -> Result<(), &'static str> {
        let tx_in_count: u128 = 1;
        let tx_out_count: u128 = 1;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generate_data_stream(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        let tx_out: Vec<TxOut> = create_txout(tx_out_count);
        assert_eq!(transaction.tx_out[0], tx_out[0]);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_returns_expected_lock_time() -> Result<(), &'static str> {
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generate_data_stream(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(transaction.lock_time, lock_time);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_returns_expected_txin_size() -> Result<(), &'static str> {
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generate_data_stream(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(transaction.tx_in.len(), tx_in_count as usize);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_returns_expected_txin_vector() -> Result<(), &'static str> {
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generate_data_stream(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        let tx_in: Vec<TxIn> = create_txin(tx_in_count);
        assert_eq!(transaction.tx_in, tx_in);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_returns_expected_txout_vector() -> Result<(), &'static str> {
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generate_data_stream(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        let tx_out: Vec<TxOut> = create_txout(tx_out_count);
        assert_eq!(transaction.tx_out, tx_out);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_two_transactions_returns_expected_length() -> Result<(), &'static str> {
        let tx_in_count: u128 = 1;
        let tx_out_count: u128 = 1;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let mut bytes = generate_data_stream(version, tx_in_count, tx_out_count, lock_time);
        let bytes2 = generate_data_stream(version, tx_in_count, tx_out_count, lock_time);
        bytes.extend_from_slice(&bytes2[0..bytes2.len()]);
        let mut offset: usize = 0;
        let transactions: Vec<Transaction> =
            Transaction::unmarshalling_transactions(&bytes, 2, &mut offset)?;
        assert_eq!(transactions.len(), 2);
        Ok(())
    }
}