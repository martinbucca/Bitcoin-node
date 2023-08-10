use crate::transactions::tx_out::TxOut;

#[derive(Debug, Clone)]
/// Stores the hash of the transaction and an array with the unspent TxOut, referring to that transaction
/// The tuple stores the TxOut and the index in which it is located in the tx
pub struct UtxoTuple {
    pub hash: [u8; 32],
    pub utxo_set: Vec<(TxOut, usize)>,
}

impl UtxoTuple {

    /// Creates a new UtxoTuple
    pub fn new(hash: [u8; 32], utxo_set: Vec<(TxOut, usize)>) -> Self {
        UtxoTuple { hash, utxo_set }
    }

    /// Returns the utxoTuple with the TxOut that reference the received address
    /// If it does not find any, returns None
    pub fn referenced_utxos(&self, address: &str) -> Option<UtxoTuple> {
        let hash = self.hash;
        let mut utxo_set: Vec<(TxOut, usize)> = Vec::new();
        for utxo in &self.utxo_set {
            match utxo.0.get_address() {
                Ok(value) => {
                    if *address == value {
                        utxo_set.push(utxo.clone());
                    }
                }
                Err(_) => {
                    continue;
                }
            }
        }
        if utxo_set.is_empty() {
            return None;
        }
        Some(UtxoTuple { hash, utxo_set })
    }

    /// Returns the amount in satoshis of the TxOut of the Utxo
    pub fn balance(&self) -> i64 {
        let mut balance = 0;
        for utxo in &self.utxo_set {
            balance += utxo.0.value();
        }
        balance
    }

    /// Returns the hash of the transaction
    pub fn hash(&self) -> [u8; 32] {
        self.hash
    }

    /// It is used to generate the txIn when creating a new transaction
    /// it can happen that a transaction has more than one outpoint referencing the utxos of that
    /// transaction that is why the need for this function
    pub fn get_indexes_from_utxos(&self) -> Vec<usize> {
        let mut indexes = Vec::new();
        for utxo in &self.utxo_set {
            indexes.push(utxo.1);
        }
        indexes
    }

    /// Receives the total amount to spend, and the amount that has already been raised
    /// Removes the necessary utxos until reaching the total amount and returns them in a new UtxoTuple
    pub fn utxos_to_spend(&mut self, value: i64, partial_amount: &mut i64) -> UtxoTuple {
        let mut utxos_to_spend = Vec::new();
        let mut position: usize = 0;
        let length: usize = self.utxo_set.len();
        while position < length {
            *partial_amount += self.utxo_set[position].0.value();
            utxos_to_spend.push(self.utxo_set[position].clone());
            if *partial_amount > value {
                break;
            }
            position += 1;
        }
        Self::new(self.hash, utxos_to_spend)
    }

    /// Search the utxo that corresponds to the received hash and index.
    /// Returns its pub key script in bytes format
    pub fn find(&self, previous_hash: [u8; 32], previous_index: usize) -> Option<&Vec<u8>> {
        if self.hash != previous_hash {
            return None;
        }
        for utxo in &self.utxo_set {
            if utxo.1 == previous_index {
                return Some(utxo.0.get_pub_key_script());
            }
        }
        None
    }

    /// Removes the output that contains the received index.
    pub fn remove_utxo(&mut self, output_index: usize) {
        for index in 0..self.utxo_set.len() {
            if self.utxo_set[index].1 == output_index {
                self.utxo_set.remove(index);
                break;
            }
        }
    }
}
