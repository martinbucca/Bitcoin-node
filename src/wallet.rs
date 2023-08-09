use std::{
    error::Error,
    io,
    sync::{Arc, RwLock},
};

use gtk::glib;

use crate::{
    account::Account,
    blocks::{
        block::Block,
        block_header::BlockHeader,
        utils_block::{make_merkle_proof, string_to_bytes},
    },
    custom_errors::NodeCustomErrors,
    gtk::ui_events::{send_event_to_ui, UIEvent},
    node::Node,
    transactions::transaction::Transaction,
};

#[derive(Debug, Clone)]
/// Represents the wallet. It has a node and a list of accounts. It also has the index of the current account.
pub struct Wallet {
    pub node: Node,
    pub current_account_index: Option<usize>,
    pub accounts: Arc<RwLock<Vec<Account>>>,
}

impl Wallet {
    /// Creates the wallet. Initializes the node with the reference of the wallet accounts
    pub fn new(node: Node) -> Result<Self, NodeCustomErrors> {
        let mut wallet = Wallet {
            node,
            current_account_index: None,
            accounts: Arc::new(RwLock::new(Vec::new())),
        };
        wallet.node.set_accounts(wallet.accounts.clone())?;
        Ok(wallet)
    }

    /// Makes a transaction with the current account of the wallet and broadcasts it.
    /// Receives the address of the receiver, amount and fee.
    /// Returns an error if something fails.
    pub fn make_transaction(
        &self,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        address_receiver: &str,
        amount: i64,
        fee: i64,
    ) -> Result<(), Box<dyn Error>> {
        let account_index = match self.current_account_index {
            Some(index) => index,
            None => {
                return Err(Box::new(std::io::Error::new(
                    io::ErrorKind::Other,
                    "Error trying to make transaction. No account selected",
                )));
            }
        };
        validate_transaction_data(amount, fee)?;
        let transaction: Transaction = self
            .accounts
            .write()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?[account_index]
            .make_transaction(address_receiver, amount, fee)?;
        self.node.broadcast_tx(transaction.hash())?;
        send_event_to_ui(ui_sender, UIEvent::NewPendingTx());
        Ok(())
    }

    /// Adds an account to the wallet.
    /// Returns an error if the keys entered are invalid and sends the error to the UI.
    /// If the account is added correctly, it sends an event to the UI to show it.
    pub fn add_account(
        &mut self,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        wif_private_key: String,
        address: String,
    ) -> Result<(), NodeCustomErrors> {
        let mut account = Account::new(wif_private_key, address).map_err(|err| {
            send_event_to_ui(ui_sender, UIEvent::AddAccountError(err.to_string()));
            NodeCustomErrors::UnmarshallingError(err.to_string())
        })?;
        self.load_data(&mut account)
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?;
        self.accounts
            .write()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .push(account.clone());
        send_event_to_ui(ui_sender, UIEvent::AccountAddedSuccesfully(account));
        Ok(())
    }

    /// Loads the respective utxos associated with the account
    fn load_data(&self, account: &mut Account) -> Result<(), Box<dyn Error>> {
        let address = account.get_address().clone();
        let utxos_to_account = self.node.utxos_referenced_to_account(&address)?;
        account.load_utxos(utxos_to_account);
        Ok(())
    }

    /// Shows the balance of the accounts.
    pub fn show_accounts_balance(&self) -> Result<(), Box<dyn Error>> {
        if self
            .accounts
            .read()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .is_empty()
        {
            println!("No accounts in the wallet");
        }
        for account in self
            .accounts
            .write()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .iter()
        {
            println!(
                "Account: {} - Balance: {:.8} tBTC",
                account.address,
                account.balance() as f64 / 1e8
            );
        }
        Ok(())
    }

    /// Changes the index of the current account of the wallet. If an index out of range is passed, it returns an error.
    /// If it is changed correctly, it sends an event to the UI to update it.
    pub fn change_account(
        &mut self,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        index_of_new_account: usize,
    ) -> Result<(), Box<dyn Error>> {
        if index_of_new_account
            >= self
                .accounts
                .read()
                .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                .len()
        {
            return Err(Box::new(std::io::Error::new(
                io::ErrorKind::Other,
                "Error trying to change account. Index out of bounds",
            )));
        }
        self.current_account_index = Some(index_of_new_account);
        let new_account = self
            .accounts
            .read()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?[index_of_new_account]
            .clone();
        send_event_to_ui(ui_sender, UIEvent::AccountChanged(new_account));
        Ok(())
    }

    /// Shows the indexes of the accounts
    pub fn show_indexes_of_accounts(&self) -> Result<(), Box<dyn Error>> {
        if self
            .accounts
            .read()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .is_empty()
        {
            println!("There are no accounts in the wallet. It is not possible to make a transaction!");
            return Err(Box::new(std::io::Error::new(
                io::ErrorKind::Other,
                "There are no accounts in the wallet. It is not possible to make a transaction!",
            )));
        }
        println!("ACCOUNT INDEXES:");
        for (index, account) in self
            .accounts
            .read()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .iter()
            .enumerate()
        {
            println!("{}: {}", index, account.address);
        }
        println!();
        Ok(())
    }

    /// Request the node the proof of inclusion of the transaction
    /// Receives the hash of the transaction and the block where it is.
    /// Evaluates the POI and returns true or false
    pub fn tx_proof_of_inclusion(
        &self,
        block_hash_hex: String,
        tx_hash_hex: String,
    ) -> Result<bool, Box<dyn Error>> {
        let mut block_hash: [u8; 32] = string_to_bytes(&block_hash_hex)?;
        let mut tx_hash: [u8; 32] = string_to_bytes(&tx_hash_hex)?;
        block_hash.reverse();
        tx_hash.reverse();

        let poi = self.node.merkle_proof_of_inclusion(&block_hash, &tx_hash)?;

        let hashes = match poi {
            Some(value) => value,
            None => return Ok(false),
        };
        Ok(make_merkle_proof(&hashes, &tx_hash))
    }

    /// Returns the current account of the wallet
    /// If there is no current account returns None
    pub fn get_current_account(&self) -> Option<Account> {
        if let Some(index) = self.current_account_index {
            return Some(
                self.accounts
                    .read()
                    .map_err(|err| NodeCustomErrors::LockError(err.to_string()))
                    .unwrap()[index]
                    .clone(),
            );
        }
        None
    }

    /// Returns a list with the transactions of the current account
    /// If there is no current account returns None
    pub fn get_transactions(&self) -> Option<Vec<(String, Transaction, i64)>> {
        if let Some(index) = self.current_account_index {
            match self
                .accounts
                .read()
                .map_err(|err| NodeCustomErrors::LockError(err.to_string()))
                .unwrap()[index]
                .get_transactions()
            {
                Ok(transactions) => return Some(transactions),
                Err(_) => return None,
            }
        }
        None
    }
    /// Search a block in the blockchain
    /// Receives the hash of the block in hex format
    /// Returns the block if found, None otherwise
    pub fn search_block(&self, hash: [u8; 32]) -> Option<Block> {
        self.node.search_block(hash)
    }

    /// Search a header in the blockchain
    /// Receives the hash of the header in hex format
    /// Returns the header if found, None otherwise
    pub fn search_header(&self, hash: [u8; 32]) -> Option<(BlockHeader, usize)> {
        self.node.search_header(hash)
    }
}

/// Validates that the amount and fee are greater than zero
fn validate_transaction_data(amount: i64, fee: i64) -> Result<(), Box<dyn Error>> {
    if (amount + fee) <= 0 {
        return Err(Box::new(std::io::Error::new(
            io::ErrorKind::Other,
            "Error trying to make transaction. Amount and fee must be greater than zero",
        )));
    }
    Ok(())
}
