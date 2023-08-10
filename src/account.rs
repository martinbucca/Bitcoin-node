use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::sync::Arc;
use std::sync::RwLock;

use crate::address_decoder;
use crate::custom_errors::NodeCustomErrors;
use crate::transactions::transaction::Transaction;
use crate::utxo_tuple::UtxoTuple;
#[derive(Debug, Clone)]
/// Represents a bitcoin account.
/// Stores the compressed address and the private key (compressed or not).
/// Also stores the utxos of the account, pending and confirmed transactions.
pub struct Account {
    pub private_key: String,
    pub address: String,
    pub utxo_set: Vec<UtxoTuple>,
    pub pending_transactions: Arc<RwLock<Vec<Transaction>>>,
    pub confirmed_transactions: Arc<RwLock<Vec<Transaction>>>,
}

type TransactionInfo = (String, Transaction, i64);
impl Account {
    /// Receives the address in compressed format and the WIF private key, either in 
    /// compressed or uncompressed format.
    pub fn new(wif_private_key: String, address: String) -> Result<Account, Box<dyn Error>> {
        let raw_private_key = address_decoder::decode_wif_private_key(wif_private_key.as_str())?;

        address_decoder::validate_address_private_key(&raw_private_key, &address)?;
        Ok(Account {
            private_key: wif_private_key,
            address,
            utxo_set: Vec::new(),
            pending_transactions: Arc::new(RwLock::new(Vec::new())),
            confirmed_transactions: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Returns the compressed public key (33 bytes) from the private key.
    pub fn get_pubkey_compressed(&self) -> Result<[u8; 33], Box<dyn Error>> {
        address_decoder::get_pubkey_compressed(&self.private_key)
    }
    /// Returns the private key decoded in bytes format.
    pub fn get_private_key(&self) -> Result<[u8; 32], Box<dyn Error>> {
        address_decoder::decode_wif_private_key(self.private_key.as_str())
    }

    /// Returns the address of the account.
    pub fn get_address(&self) -> &String {
        &self.address
    }

    /// Stores the utxos in the account.
    pub fn load_utxos(&mut self, utxos: Vec<UtxoTuple>) {
        self.utxo_set = utxos;
    }

    /// Compares the amount received with the account balance.
    /// Returns true if the balance is greater. Otherwise false.
    pub fn has_balance(&self, value: i64) -> bool {
        self.balance() > value
    }

    /// Returns the balance of the account.
    pub fn balance(&self) -> i64 {
        let mut balance: i64 = 0;
        for utxo in &self.utxo_set {
            balance += utxo.balance();
        }
        balance
    }
    /// Returns a vec with the utxos to be spent in a new transaction, according to the amount received.
    fn get_utxos_for_amount(&mut self, value: i64) -> Vec<UtxoTuple> {
        let mut utxos_to_spend = Vec::new();
        let mut partial_amount: i64 = 0;
        let mut position: usize = 0;
        let length: usize = self.utxo_set.len();
        while position < length {
            if (partial_amount + self.utxo_set[position].balance()) < value {
                partial_amount += self.utxo_set[position].balance();
                utxos_to_spend.push(self.utxo_set[position].clone());
                // As the tx is not confirmed yet, it is not necessary to remove them
            } else {
                utxos_to_spend
                    .push(self.utxo_set[position].utxos_to_spend(value, &mut partial_amount));
                break;
            }
            position += 1;
        }
        utxos_to_spend
    }

    /// Add the transaction to the list of pending transactions.
    fn add_transaction(&self, transaction: Transaction) -> Result<(), Box<dyn Error>> {
        let mut aux = self
            .pending_transactions
            .write()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?;
        aux.push(transaction);
        Ok(())
    }

    /// Makes the transaction with the amount received. 
    /// Returns the hash of the transaction so that the node sends that hash to the remaining nodes in the network.
    pub fn make_transaction(
        &mut self,
        address_receiver: &str,
        amount: i64,
        fee: i64,
    ) -> Result<Transaction, Box<dyn Error>> {
        address_decoder::validate_address(address_receiver)?;
        if !self.has_balance(amount + fee) {
            return Err(Box::new(std::io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "The balance of the account {} has less than {} satoshis",
                    self.address,
                    amount + fee,
                ),
            )));
        }
        // The amount is already known, now we need to get the utxos to spend
        let utxos_to_spend: Vec<UtxoTuple> = self.get_utxos_for_amount(amount + fee);
        let change_address: &str = self.address.as_str();
        let mut unsigned_transaction = Transaction::generate_unsigned_transaction(
            address_receiver,
            change_address,
            amount,
            fee,
            &utxos_to_spend,
        )?;
        unsigned_transaction.sign(self, &utxos_to_spend)?;
        unsigned_transaction.validate(&utxos_to_spend)?;
        self.add_transaction(unsigned_transaction.clone())?;
        Ok(unsigned_transaction)
    }

    /// Receives the utxo_set, iterates it and sets the account utxo_set.
    pub fn set_utxos(
        &mut self,
        utxo_set: Arc<RwLock<HashMap<[u8; 32], UtxoTuple>>>,
    ) -> Result<(), Box<dyn Error>> {
        let mut account_utxo_set: Vec<UtxoTuple> = Vec::new();
        for utxo in utxo_set
            .read()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .values()
        {
            let aux_utxo = utxo.referenced_utxos(&self.address);
            let utxo_to_push = match aux_utxo {
                Some(value) => value,
                None => continue,
            };
            account_utxo_set.push(utxo_to_push);
        }
        self.utxo_set = account_utxo_set;
        Ok(())
    }

    /// Returns the pending and confirmed transactions of the account.
    /// Returns a list of tuples with the state, transaction and amount sent by the account.
    pub fn get_transactions(&self) -> Result<Vec<TransactionInfo>, Box<dyn Error>> {
        let mut transactions: Vec<(String, Transaction, i64)> = Vec::new();
        // iterate pending transactions
        for tx in self
            .pending_transactions
            .read()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .iter()
        {
            transactions.push((
                "Pending".to_string(),
                tx.clone(),
                tx.amount_spent_by_account(&self.address)?,
            ));
        }

        for tx in self
            .confirmed_transactions
            .read()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .iter()
        {
            transactions.push((
                "Confirmed".to_string(),
                tx.clone(),
                tx.amount_spent_by_account(&self.address)?,
            ));
        }

        Ok(transactions)
    }
}

/// Converts the bytes to hexadecimal and returns it
pub fn bytes_to_hex_string(bytes: &[u8]) -> String {
    let hex_chars: Vec<String> = bytes.iter().map(|byte| format!("{:02x}", byte)).collect();
    hex_chars.join("")
}

#[cfg(test)]
mod test {

    use crate::account::Account;
    use std::{
        error::Error,
        io,
        sync::{Arc, RwLock},
    };

    /// Converts the received hexadecimal string into bytes
    fn string_to_33_bytes(input: &str) -> Result<[u8; 33], Box<dyn Error>> {
        if input.len() != 66 {
            return Err(Box::new(std::io::Error::new(
                io::ErrorKind::Other,
                "The received string is invalid. It doesn't have the correct length",
            )));
        }

        let mut result = [0; 33];
        for i in 0..33 {
            let byte_str = &input[i * 2..i * 2 + 2];
            result[i] = u8::from_str_radix(byte_str, 16)?;
        }

        Ok(result)
    }

    #[test]
    fn test_account_generation_with_compressed_wif_is_successful() {
        let address_expected: String = String::from("mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV");
        let private_key: String =
            String::from("cMoBjaYS6EraKLNqrNN8DvN93Nnt6pJNfWkYM8pUufYQB5EVZ7SR");
        let account_result = Account::new(private_key, address_expected);
        assert!(account_result.is_ok());
    }

    #[test]
    fn test_account_generation_with_uncompressed_wif_is_successful() {
        let address_expected: String = String::from("mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV");
        let private_key: String =
            String::from("91dkDNCCaMp2f91sVQRGgdZRw1QY4aptaeZ4vxEvuG5PvZ9hftJ");
        let account_result = Account::new(private_key, address_expected);
        assert!(account_result.is_ok());
    }

    #[test]
    fn test_account_generation_with_incorrect_wif_fails() {
        let address_expected: String = String::from("mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV");
        let private_key: String =
            String::from("K1dkDNCCaMp2f91sVQRGgdZRw1QY4aptaeZ4vxEvuG5PvZ9hftJ");
        let account_result = Account::new(private_key, address_expected);
        assert!(account_result.is_err());
    }

    #[test]
    fn test_user_returns_expected_compressed_public_key() -> Result<(), Box<dyn Error>> {
        let address = String::from("mpzx6iZ1WX8hLSeDRKdkLatXXPN1GDWVaF");
        let private_key = String::from("cQojsQ5fSonENC5EnrzzTAWSGX8PB4TBh6GunBxcCdGMJJiLULwZ");
        let user = Account {
            private_key,
            address,
            utxo_set: Vec::new(),
            pending_transactions: Arc::new(RwLock::new(Vec::new())),
            confirmed_transactions: Arc::new(RwLock::new(Vec::new())),
        };
        let expected_pubkey = string_to_33_bytes(
            "0345EC0AA86BAF64ED626EE86B4A76C12A92D5F6DD1C1D6E4658E26666153DAFA6",
        )?;
        assert_eq!(user.get_pubkey_compressed()?, expected_pubkey);
        Ok(())
    }

    #[test]
    fn test_transaction_to_invalid_address_fails() -> Result<(), Box<dyn Error>> {
        let address_expected: String = String::from("mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV");
        let private_key: String =
            String::from("cMoBjaYS6EraKLNqrNN8DvN93Nnt6pJNfWkYM8pUufYQB5EVZ7SR");
        let mut account = Account::new(private_key, address_expected)?;
        let transaction_result =
            account.make_transaction("mocD12x6BV3qK71FwG98h5VWZ4qVsbaoi8", 1000, 10);
        assert!(transaction_result.is_err());
        Ok(())
    }
}
