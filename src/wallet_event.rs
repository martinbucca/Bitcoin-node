use crate::{
    custom_errors::NodeCustomErrors,
    gtk::ui_events::{send_event_to_ui, UIEvent},
    wallet::Wallet,
};
use gtk::glib;
use std::sync::mpsc::Receiver;

type Address = String;
type WifPrivateKey = String;
type AccountIndex = usize;
type Amount = i64;
type Fee = i64;
type BlockHash = [u8; 32];
type BlockHashString = String;
type TransactionHash = String;

/// Represents the events that the UI sends to the wallet
pub enum WalletEvent {
    Start,
    AddAccountRequest(WifPrivateKey, Address),
    MakeTransaction(Address, Amount, Fee),
    PoiOfTransactionRequest(BlockHashString, TransactionHash),
    Finish,
    ChangeAccount(AccountIndex),
    GetAccountRequest,
    GetTransactionsRequest,
    SearchBlock(BlockHash),
    SearchHeader(BlockHash),
}

/// Received a sender that sends events to the UI, a receiver that receives events from the UI and a wallet
/// It is responsible for handling UI events and calling the corresponding methods of the wallet
/// to perform the corresponding actions. Sends events to the UI to show the results
pub fn handle_ui_request(
    ui_sender: &Option<glib::Sender<UIEvent>>,
    rx: Receiver<WalletEvent>,
    wallet: &mut Wallet,
) {
    for event in rx {
        match event {
            WalletEvent::AddAccountRequest(wif, address) => {
                handle_add_account(ui_sender, wallet, wif, address);
            }
            WalletEvent::ChangeAccount(account_index) => {
                handle_change_account(ui_sender, wallet, account_index);
            }
            WalletEvent::GetAccountRequest => {
                handle_get_account(ui_sender, wallet);
            }
            WalletEvent::MakeTransaction(address, amount, fee) => {
                handle_make_transaction(ui_sender, wallet, address, amount, fee)
            }
            WalletEvent::PoiOfTransactionRequest(block_hash, transaction_hash) => {
                handle_poi(ui_sender, wallet, block_hash, transaction_hash);
            }
            WalletEvent::SearchBlock(block_hash) => {
                handle_search_block(ui_sender, wallet, block_hash);
            }
            WalletEvent::SearchHeader(block_hash) => {
                handle_search_header(ui_sender, wallet, block_hash);
            }
            WalletEvent::GetTransactionsRequest => {
                handle_get_transactions(ui_sender, wallet);
            }
            WalletEvent::Finish => {
                break;
            }
            _ => (),
        }
    }
}

/// Receives a sender that sends events to the UI, a wallet, the private-key wif and an address
/// It is responsible for calling the method of the wallet that adds an account. In case of error when adding the account
/// sends an event to the UI to show the error
fn handle_add_account(
    ui_sender: &Option<glib::Sender<UIEvent>>,
    wallet: &mut Wallet,
    private_key_wif: String,
    address: String,
) {
    if let Err(NodeCustomErrors::LockError(err)) =
        wallet.add_account(ui_sender, private_key_wif, address)
    {
        send_event_to_ui(ui_sender, UIEvent::AddAccountError(err));
    }
}

/// Receives a sender that sends events to the UI, a wallet and the index of the account to change
/// It is responsible for calling the method of the wallet that changes the current account. In case of error when changing the account
/// sends an event to the UI to show the error
fn handle_change_account(
    ui_sender: &Option<glib::Sender<UIEvent>>,
    wallet: &mut Wallet,
    account_index: usize,
) {
    if let Err(err) = wallet.change_account(ui_sender, account_index) {
        send_event_to_ui(ui_sender, UIEvent::ChangeAccountError(err.to_string()));
    }
}

/// Receives a sender that sends events to the UI and a wallet
/// It is responsible for calling the method of the wallet that returns the current account. In case the account exists
/// sends an event to the UI to show the current account
fn handle_get_account(ui_sender: &Option<glib::Sender<UIEvent>>, wallet: &mut Wallet) {
    if let Some(account) = wallet.get_current_account() {
        send_event_to_ui(ui_sender, UIEvent::AccountChanged(account));
    }
}

/// Receives a sender that sends events to the UI, a wallet, an address, an amount and a fee
/// It is responsible for calling the method of the wallet that makes a transaction. In case of error when making the transaction
/// sends an event to the UI to show the error. In case the transaction is made correctly, it sends an event
/// to the UI to show that the transaction was made correctly
fn handle_make_transaction(
    ui_sender: &Option<glib::Sender<UIEvent>>,
    wallet: &mut Wallet,
    address: String,
    amount: i64,
    fee: i64,
) {
    if let Err(err) = wallet.make_transaction(ui_sender, &address, amount, fee) {
        send_event_to_ui(ui_sender, UIEvent::MakeTransactionStatus(err.to_string()));
    } else {
        send_event_to_ui(
            ui_sender,
            UIEvent::MakeTransactionStatus("The transaction was made succesfuly!".to_string()),
        );
    }
}

/// Receives a sender that sends events to the UI, a wallet, a block hash and a transaction hash
/// It is responsible for calling the method of the wallet that makes the PoI in a transaction. In case of error when making the PoI
/// sends an event to the UI to show the error. In case the PoI is made correctly, it sends an event
/// to the UI to show the result of the PoI.
fn handle_poi(
    ui_sender: &Option<glib::Sender<UIEvent>>,
    wallet: &mut Wallet,
    block_hash: String,
    transaction_hash: String,
) {
    let message;
    match wallet.tx_proof_of_inclusion(block_hash.clone(), transaction_hash.clone()) {
        Err(_) => message = "Block not found".to_string(),
        Ok(poi) => {
            if poi {
                message =
                    format!("The transaction {transaction_hash} was found on block {block_hash}");
            } else {
                message = format!(
                    "The transaction {transaction_hash} was not found on block {block_hash}"
                );
            }
        }
    }
    send_event_to_ui(ui_sender, UIEvent::POIResult(message));
}

/// Receives a sender that sends events to the UI, a wallet and a block hash
/// It is responsible for calling the method of the wallet that searches for a block by its hash. If the block exists
/// sends an event to the UI to show the block. If the block does not exist, it sends an event to the UI
/// to show that the block was not found
fn handle_search_block(
    ui_sender: &Option<glib::Sender<UIEvent>>,
    wallet: &mut Wallet,
    block_hash: [u8; 32],
) {
    if let Some(block) = wallet.search_block(block_hash) {
        send_event_to_ui(ui_sender, UIEvent::BlockFound(block));
    } else {
        send_event_to_ui(ui_sender, UIEvent::NotFound);
    }
}

/// Receives a sender that sends events to the UI, a wallet and a block hash
/// It is responsible for calling the method of the wallet that searches for a header by its hash. If the header exists
/// sends an event to the UI to show the header. If the header does not exist, it sends an event to the UI
/// to show that the header was not found
fn handle_search_header(
    ui_sender: &Option<glib::Sender<UIEvent>>,
    wallet: &mut Wallet,
    block_hash: [u8; 32],
) {
    if let Some((header, height)) = wallet.search_header(block_hash) {
        send_event_to_ui(ui_sender, UIEvent::HeaderFound(header, height));
    } else {
        send_event_to_ui(ui_sender, UIEvent::NotFound);
    }
}

/// Request the wallet to send the transactions of the current account to the UI
pub fn handle_get_transactions(ui_sender: &Option<glib::Sender<UIEvent>>, wallet: &mut Wallet) {
    if let Some(transactions) = wallet.get_transactions() {
        send_event_to_ui(ui_sender, UIEvent::UpdateTransactions(transactions));
    }
}
