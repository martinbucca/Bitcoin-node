use super::{
    block_header::BlockHeader, merkle_tree::MerkleTree, utils_block::concatenate_and_hash,
};
use crate::{
    account::Account,
    compact_size_uint::CompactSizeUint,
    custom_errors::NodeCustomErrors,
    gtk::ui_events::{send_event_to_ui, UIEvent},
    logwriter::log_writer::{write_in_log, LogSender},
    transactions::transaction::Transaction,
    utxo_tuple::UtxoTuple,
};
use gtk::glib;
use std::{
    collections::HashMap,
    error::Error,
    sync::{Arc, RwLock},
};

#[derive(Debug, Clone)]
/// Represents a block of the bitcoin protocol.
pub struct Block {
    pub block_header: BlockHeader,
    pub txn_count: CompactSizeUint,
    pub txn: Vec<Transaction>,
}

impl Block {
    /// Creates a new Block with the received fields.
    pub fn new(
        block_header: BlockHeader,
        txn_count: CompactSizeUint,
        txn: Vec<Transaction>,
    ) -> Block {
        Block {
            block_header,
            txn_count,
            txn,
        }
    }

    /// Receives a vector of bytes, deserializes it and returns the block.
    /// Updates the offset according to the amount of bytes it read from the string.
    pub fn unmarshalling(bytes: &Vec<u8>, offset: &mut usize) -> Result<Block, &'static str> {
        let block_header: BlockHeader = BlockHeader::unmarshalling(bytes, offset)?;
        let txn_count: CompactSizeUint = CompactSizeUint::unmarshalling(bytes, offset)?;
        let amount_transaction: u64 = txn_count.decoded_value();
        let txn: Vec<Transaction> =
            Transaction::unmarshalling_transactions(bytes, amount_transaction, offset)?;
        Ok(Block {
            block_header,
            txn_count,
            txn,
        })
    }

    /// Converts the block to bytes according to the bitcoin protocol.
    /// Saves those bytes in the vector received by parameter.
    pub fn marshalling(&self, bytes: &mut Vec<u8>) {
        self.block_header.marshalling(bytes);
        bytes.extend_from_slice(&self.txn_count.marshalling());
        for tx in &self.txn {
            tx.marshalling(bytes);
        }
    }

    /// Valida el bloque. Primero realiza la proof of work y
    /// Luego realiza la proof of inclusion sobre su lista de transacciones
    /// Validates the block. First performs the proof of work and
    /// then it performs the proof of inclusion on its list of transactions.
    pub fn validate(&self) -> (bool, &'static str) {
        //proof of work
        if !self.block_header.validate() {
            return (false, "The block does not meet the proof of work");
        }
        //proof of inclusion
        let merkle_root_hash: [u8; 32] = self.generate_merkle_root();
        if !self
            .block_header
            .is_same_merkle_root_hash(&merkle_root_hash)
        {
            return (
                false,
                "The merkle root generated by the block does not match the one in the header",
            );
        }
        let mut weight = Vec::new();
        self.marshalling(&mut weight);
        // Check that the block does not exceed 1 MB
        if weight.len() > 1048576 {
            return (false, "The block exceeds 1 MB");
        }
        (true, "Valid block")
    }

    /// Generates the merkle root root from the hashes of the transactions (tx_id).
    /// Reduces the elements of the tx_id vector, groups them in pairs, hashes them and saves them again
    /// in a vector which will be processed recursively until the merkle root hash is obtained.
    pub fn recursive_generation_merkle_root(vector: Vec<[u8; 32]>) -> [u8; 32] {
        let vec_length: usize = vector.len();
        if vec_length == 1 {
            return vector[0];
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
        // If the length of the vector is odd, the last element must be concatenated with itself
        // and then the hash function applied
        if (vec_length % 2) != 0 {
            upper_level.push(concatenate_and_hash(
                vector[current_position - 1],
                vector[current_position - 1],
            ));
        }
        Self::recursive_generation_merkle_root(upper_level)
    }

    /// Genreates the merkle root 
    pub fn generate_merkle_root(&self) -> [u8; 32] {
        let mut merkle_transactions: Vec<[u8; 32]> = Vec::new();
        for tx in &self.txn {
            merkle_transactions.push(tx.hash());
        }
        Self::recursive_generation_merkle_root(merkle_transactions)
    }
    pub fn is_same_block(&self, block_id: &[u8; 32]) -> bool {
        self.block_header.hash() == *block_id
    }

    /// Updates the utxo_set received by parameter.
    /// Processes the block transactions. Adds the new utxos and removes the spent ones.
    pub fn give_me_utxos(
        &self,
        utxo_set: Arc<RwLock<HashMap<[u8; 32], UtxoTuple>>>,
    ) -> Result<(), Box<dyn Error>> {
        for tx in &self.txn {
            if tx.is_coinbase_transaction() {
                // As it is a coinbase, being the first tx, only the utxos of this transaction will be loaded 
                tx.load_utxos(utxo_set.clone())?;
            } else {
                // Remove the utxos used by this tx
                tx.remove_utxos(utxo_set.clone())?;
                // Then load the utxos of this tx so that in the next iteration
                // those that are used are removed
                tx.load_utxos(utxo_set.clone())?;
            }
        }
        Ok(())
    }

    /// Generates the merkle proof of inclusion of the transaction received by parameter.
    pub fn merkle_proof_of_inclusion(
        &self,
        tx_id_to_find: &[u8; 32],
    ) -> Option<Vec<([u8; 32], bool)>> {
        let mut hashes: Vec<[u8; 32]> = Vec::new();
        for tx in &self.txn {
            hashes.push(tx.hash());
        }
        let merkle_tree = MerkleTree::new(&hashes);
        merkle_tree.merkle_proof_of_inclusion(*tx_id_to_find)
    }

    /// Returns a string representing the block hash in hexadecimal.
    /// The format is like the one used by web explorers (e.g. https://blockstream.info/testnet/)
    /// to show blocks
    pub fn hex_hash(&self) -> String {
        self.block_header.hex_hash()
    }

    /// Returns a string representing the merkle root hash in hexadecimal.
    pub fn hex_merkle_root_hash(&self) -> String {
        self.block_header.hex_merkle_root_hash()
    }

    /// Notifies if the block contains a transaction that was pending.
    /// Checks the block transactions and compares them with the pending transactions of the accounts
    pub fn contains_pending_tx(
        &self,
        log_sender: &LogSender,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        accounts: Arc<RwLock<Arc<RwLock<Vec<Account>>>>>,
    ) -> Result<(), NodeCustomErrors> {
        for tx in &self.txn {
            for account in &*accounts
                .read()
                .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                .read()
                .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            {
                if account
                    .pending_transactions
                    .read()
                    .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                    .contains(tx)
                {
                    println!(
                        "THE BLOCK {} \nCONTAINS THE CONFIRMED TRANSACTION {} \nFROM THE ACCOUNT {}\n",
                        self.hex_hash(),
                        tx.hex_hash(),
                        account.address
                    );
                    let pending_transaction_index = account
                        .pending_transactions
                        .read()
                        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                        .iter()
                        .position(|pending_tx| pending_tx.hash() == tx.hash());
                    if let Some(pending_transaction_index) = pending_transaction_index {
                        let confirmed_tx = account
                            .pending_transactions
                            .write()
                            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                            .remove(pending_transaction_index);
                        account
                            .confirmed_transactions
                            .write()
                            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                            .push(confirmed_tx.clone());
                        write_in_log(
                            &log_sender.info_log_sender,
                            format!(
                                "ACCOUNT: {}: NEW TRANSACTION CONFIRMED {} IN BLOCK --{}--",
                                account.address,
                                confirmed_tx.hex_hash(),
                                self.hex_hash()
                            )
                            .as_str(),
                        );
                        send_event_to_ui(
                            ui_sender,
                            UIEvent::ShowConfirmedTransaction(
                                self.clone(),
                                account.clone(),
                                tx.clone(),
                            ),
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Returns the block hash.
    pub fn hash(&self) -> [u8; 32] {
        self.block_header.hash()
    }

    /// Returns a string representing the block timestamp in local date format.
    pub fn local_time(&self) -> String {
        self.block_header.local_time()
    }

    /// Returns the block height.
    pub fn get_height(&self) -> u32 {
        self.txn[0].get_height()
    }
}

#[cfg(test)]
mod test {
    use crate::{
        blocks::{block_header::BlockHeader, utils_block::concatenate_and_hash},
        compact_size_uint::CompactSizeUint,
        transactions::{
            outpoint::Outpoint, script::sig_script::SigScript, transaction::Transaction,
            tx_in::TxIn, tx_out::TxOut,
        },
    };
    use std::{error::Error, io, vec};

    use super::Block;

    /// Converts the str received in hexadecimal, to bytes.
    fn string_to_bytes(input: &str) -> Result<[u8; 32], Box<dyn Error>> {
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
    /// Converts bytes to hexadecimal.
    pub fn bytes_to_hex_string(bytes: &[u8]) -> String {
        let hex_chars: Vec<String> = bytes.iter().map(|byte| format!("{:02x}", byte)).collect();

        hex_chars.join("")
    }

    fn create_txins(cantidad: u128) -> Vec<TxIn> {
        let mut tx_in: Vec<TxIn> = Vec::new();
        for _i in 0..cantidad {
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

    fn create_txouts(cantidad: u128) -> Vec<TxOut> {
        let mut tx_out: Vec<TxOut> = Vec::new();
        for _i in 0..cantidad {
            let value: i64 = 43;
            let pk_script_bytes: CompactSizeUint = CompactSizeUint::new(0);
            let pk_script: Vec<u8> = Vec::new();
            tx_out.push(TxOut::new(value, pk_script_bytes, pk_script));
        }
        tx_out
    }

    fn create_transaction(
        version: i32,
        tx_in_count: u128,
        tx_out_count: u128,
        lock_time: u32,
    ) -> Transaction {
        // version settings
        let version: i32 = version;
        // tx_in_count settings
        let txin_count = CompactSizeUint::new(tx_in_count);
        // tx_in settings
        let tx_in: Vec<TxIn> = create_txins(tx_in_count);
        // tx_out_count settings
        let txout_count = CompactSizeUint::new(tx_out_count);
        // tx_out settings
        let tx_out: Vec<TxOut> = create_txouts(tx_out_count);
        //lock_time settings
        let lock_time: u32 = lock_time;
        let transaction: Transaction =
            Transaction::new(version, txin_count, tx_in, txout_count, tx_out, lock_time);
        transaction
    }

    #[test]
    fn test_unmarshaling_block_generates_expected_block_header() -> Result<(), &'static str> {
        let mut bytes_to_read: Vec<u8> = Vec::new();
        let block_header: BlockHeader = BlockHeader {
            version: (0x30201000),
            previous_block_header_hash: ([1; 32]),
            merkle_root_hash: ([2; 32]),
            time: (0x90807060),
            n_bits: (0x04030201),
            nonce: (0x30),
        };
        block_header.marshalling(&mut bytes_to_read);
        let txn_count_bytes: CompactSizeUint = CompactSizeUint::new(1);
        let txn_count: Vec<u8> = txn_count_bytes.marshalling();
        bytes_to_read.extend_from_slice(&txn_count);
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let tx: Transaction = create_transaction(version, tx_in_count, tx_out_count, lock_time);
        tx.marshalling(&mut bytes_to_read);
        let mut offset: usize = 0;
        let block: Block = Block::unmarshalling(&bytes_to_read, &mut offset)?;
        assert_eq!(block.block_header, block_header);
        Ok(())
    }

    #[test]
    fn test_unmarshaling_block_generates_expected_txn_count() -> Result<(), &'static str> {
        let mut bytes_to_read: Vec<u8> = Vec::new();
        let block_header: BlockHeader = BlockHeader {
            version: (0x30201000),
            previous_block_header_hash: ([1; 32]),
            merkle_root_hash: ([2; 32]),
            time: (0x90807060),
            n_bits: (0x04030201),
            nonce: (0x30),
        };
        block_header.marshalling(&mut bytes_to_read);
        let txn_count_bytes: CompactSizeUint = CompactSizeUint::new(1);
        let txn_count: Vec<u8> = txn_count_bytes.marshalling();
        bytes_to_read.extend_from_slice(&txn_count);
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let tx: Transaction = create_transaction(version, tx_in_count, tx_out_count, lock_time);
        tx.marshalling(&mut bytes_to_read);
        let mut offset: usize = 0;
        let block: Block = Block::unmarshalling(&bytes_to_read, &mut offset)?;
        assert_eq!(block.txn_count, txn_count_bytes);
        Ok(())
    }

    #[test]
    fn test_unmarshaling_block_generates_expected_transaction() -> Result<(), &'static str> {
        let mut bytes_to_read: Vec<u8> = Vec::new();
        let block_header: BlockHeader = BlockHeader {
            version: (0x30201000),
            previous_block_header_hash: ([1; 32]),
            merkle_root_hash: ([2; 32]),
            time: (0x90807060),
            n_bits: (0x04030201),
            nonce: (0x30),
        };
        block_header.marshalling(&mut bytes_to_read);
        let txn_count_bytes: CompactSizeUint = CompactSizeUint::new(1);
        let txn_count: Vec<u8> = txn_count_bytes.marshalling();
        bytes_to_read.extend_from_slice(&txn_count);
        let tx_in_count: u128 = 1;
        let tx_out_count: u128 = 1;
        let version: i32 = 100;
        let lock_time: u32 = 3;
        let tx: Transaction = create_transaction(version, tx_in_count, tx_out_count, lock_time);
        tx.marshalling(&mut bytes_to_read);
        let mut offset: usize = 0;
        let block: Block = Block::unmarshalling(&bytes_to_read, &mut offset)?;
        assert_eq!(block.txn[0], tx);
        Ok(())
    }


    #[test]
    fn test_merkle_root_of_block_with_2_transactions_is_generated_correctly() {
        let block_header: BlockHeader = BlockHeader {
            version: (0x30201000),
            previous_block_header_hash: ([1; 32]),
            merkle_root_hash: ([2; 32]),
            time: (0x90807060),
            n_bits: (0x04030201),
            nonce: (0x30),
        };
        let txn_count_bytes: CompactSizeUint = CompactSizeUint::new(2);
        let mut txn: Vec<Transaction> = Vec::new();
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        txn.push(create_transaction(
            version,
            tx_in_count,
            tx_out_count,
            lock_time,
        ));
        let tx_in_count: u128 = 5;
        let tx_out_count: u128 = 3;
        let version: i32 = 34;
        let lock_time: u32 = 3;
        txn.push(create_transaction(
            version,
            tx_in_count,
            tx_out_count,
            lock_time,
        ));
        let first_hash: [u8; 32] = txn[0].hash();
        let second_hash: [u8; 32] = txn[1].hash();
        let expected_hash = concatenate_and_hash(first_hash, second_hash);
        let block: Block = Block::new(block_header, txn_count_bytes, txn);
        assert_eq!(block.generate_merkle_root(), expected_hash);
    }

    #[test]
    fn test_merkle_root_of_block_with_3_transactions_is_generated_correctly() {
        let block_header: BlockHeader = BlockHeader {
            version: (0x30201000),
            previous_block_header_hash: ([1; 32]),
            merkle_root_hash: ([2; 32]),
            time: (0x90807060),
            n_bits: (0x04030201),
            nonce: (0x30),
        };
        let txn_count_bytes: CompactSizeUint = CompactSizeUint::new(3);
        let mut txn: Vec<Transaction> = Vec::new();
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        txn.push(create_transaction(
            version,
            tx_in_count,
            tx_out_count,
            lock_time,
        ));
        let tx_in_count: u128 = 9;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 67;
        txn.push(create_transaction(
            version,
            tx_in_count,
            tx_out_count,
            lock_time,
        ));
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 2;
        let version: i32 = 39;
        let lock_time: u32 = 3;
        txn.push(create_transaction(
            version,
            tx_in_count,
            tx_out_count,
            lock_time,
        ));
        let first_hash: [u8; 32] = txn[0].hash();
        let second_hash: [u8; 32] = txn[1].hash();
        let third_hash: [u8; 32] = txn[2].hash();
        let expected_hash_1 = concatenate_and_hash(first_hash, second_hash);
        let expected_hash_2 = concatenate_and_hash(third_hash, third_hash);
        let expected_hash_final = concatenate_and_hash(expected_hash_1, expected_hash_2);
        let block: Block = Block::new(block_header, txn_count_bytes, txn);
        assert_eq!(block.generate_merkle_root(), expected_hash_final);
    }

    #[test]
    fn test_correct_generation_of_merkle_root_hash_of_mainnet_block(
    ) -> Result<(), Box<dyn Error>> {
        // Block 00000000000000127a638dfa7b517f1045217884cb986ab8f653b8be0ab37447
        // These reversals are used to convert the actual IDs since the hashes on the website
        // are provided in little-endian (LE) format
        // Link to the page: https://tbtc.bitaps.com/00000000000000127a638dfa7b517f1045217884cb986ab8f653b8be0ab37447
        let mut transactions: Vec<[u8; 32]> = Vec::new();
        let mut coinbase =
            string_to_bytes("129f32d171b2a0c4ad5fd21f7504ae483845d311214f79eb927db49dfb28b838")?;
        coinbase.reverse();
        transactions.push(coinbase);
        let mut tx_1 =
            string_to_bytes("aefeb6fb10f2f6a63a3cd4f70f1b7f8b193881a10ae5832a595e938d1630f1b9")?;
        tx_1.reverse();
        transactions.push(tx_1);
        let mut tx_2 =
            string_to_bytes("4b0d8fd869e252803909aed9642bc8af28ebd18f2c4045b9b41679eda0ff79dd")?;
        tx_2.reverse();
        transactions.push(tx_2);
        let mut tx_3 =
            string_to_bytes("dbd558c896afe59a6dce2dc26bc32f4679b336ff0b1c0f2f8aaee846c5732333")?;
        tx_3.reverse();
        transactions.push(tx_3);
        let mut tx_4 =
            string_to_bytes("88030de1d5f1b023893f8258df1796863756d99eef5c91a5528362f73497ac51")?;
        tx_4.reverse();
        transactions.push(tx_4);
        let mut merkle_root = Block::recursive_generation_merkle_root(transactions);
        merkle_root.reverse();
        let hash_generated = bytes_to_hex_string(&merkle_root);
        let hash_expected = "bc689ae06069c1381eb92aabef250bb576d8aac8aedec9b7533a37351b6dedf8";
        assert_eq!(hash_generated, hash_expected);
        Ok(())
    }
}