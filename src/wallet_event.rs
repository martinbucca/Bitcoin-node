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

/// Representa los eventos que la UI le envia a la wallet
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

/// Recibe un sender que envia eventos a la UI, un receiver que recibe eventos de la UI y una wallet
/// Se encarga de manejar los eventos de la UI y llamar a los metodos correspondientes de la wallet
/// para que realice las acciones correspondientes. Envia eventos a la UI para que muestre los resultados
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

/// Recibe un sender que envia eventos a la UI, una wallet, la private-key wif y una direccion
/// Se encarga de llamar al metodo de la wallet que agrega una cuenta. En caso de error al agregar la cuenta
/// envia un evento a la UI para que muestre el error
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

/// Recibe un sender que envia eventos a la UI, una wallet y el indice de la cuenta a cambiar
/// Se encarga de llamar al metodo de la wallet que cambia la cuenta actual. En caso de error al cambiar la cuenta
/// envia un evento a la UI para que muestre el error
fn handle_change_account(
    ui_sender: &Option<glib::Sender<UIEvent>>,
    wallet: &mut Wallet,
    account_index: usize,
) {
    if let Err(err) = wallet.change_account(ui_sender, account_index) {
        send_event_to_ui(ui_sender, UIEvent::ChangeAccountError(err.to_string()));
    }
}

/// Recibe un sender que envia eventos a la UI y una wallet
/// Se encarga de llamar al metodo de la wallet que devuelve la cuenta actual. En caso de que la cuenta exista
/// envia un evento a la UI para que muestre la cuenta actual
fn handle_get_account(ui_sender: &Option<glib::Sender<UIEvent>>, wallet: &mut Wallet) {
    if let Some(account) = wallet.get_current_account() {
        send_event_to_ui(ui_sender, UIEvent::AccountChanged(account));
    }
}

/// Recibe un sender que envia eventos a la UI, una wallet, una direccion, un monto y una comision
/// Se encarga de llamar al metodo de la wallet que realiza una transaccion. En caso de error al realizar la transaccion
/// envia un evento a la UI para que muestre el error. En caso de que la transaccion se realice correctamente envia un evento
/// a la UI para que muestre que la transaccion se realizo correctamente
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

/// Recibe un sender que envia eventos a la UI, una wallet y un hash de bloque
/// Se encarga de llamar al metodo de la wallet que busca un bloque por su hash. En caso de que el bloque exista
/// envia un evento a la UI para que muestre el bloque. En caso de que el bloque no exista envia un evento a la UI
/// para que muestre que no se encontro el bloque
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

/// Recibe un sender que envia eventos a la UI, una wallet y un hash de bloque
/// Se encarga de llamar al metodo de la wallet que busca un header por su hash. En caso de que el header exista
/// envia un evento a la UI para que muestre el header. En caso de que el header no exista envia un evento a la UI
/// para que muestre que no se encontro el header
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

/// Solicita a la wallet que envie a la UI las transacciones de la cuenta actual
pub fn handle_get_transactions(ui_sender: &Option<glib::Sender<UIEvent>>, wallet: &mut Wallet) {
    if let Some(transactions) = wallet.get_transactions() {
        send_event_to_ui(ui_sender, UIEvent::UpdateTransactions(transactions));
    }
}
