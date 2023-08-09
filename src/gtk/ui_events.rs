use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use gtk::glib;

use crate::{
    account::Account, blocks::block::Block, blocks::block_header::BlockHeader,
    transactions::transaction::Transaction,
};

type Blocks = Arc<RwLock<HashMap<[u8; 32], Block>>>;
type Headers = Arc<RwLock<Vec<BlockHeader>>>;
type Height = usize;

#[derive(Clone, Debug)]
/// Representa los eventos que la wallet le envia a la UI para que los muestre
pub enum UIEvent {
    StartHandshake,
    StartDownloadingHeaders,
    FinsihDownloadingHeaders(usize),
    StartDownloadingBlocks,
    ShowConfirmedTransaction(Block, Account, Transaction),
    AccountAddedSuccesfully(Account),
    AddAccountError(String),
    AccountChanged(Account),
    ChangeAccountError(String),
    ShowPendingTransaction(Account, Transaction),
    AddBlock(Block),
    InitializeUITabs((Headers, Blocks)),
    ActualizeHeadersDownloaded(usize),
    ActualizeBlocksDownloaded(usize, usize),
    MakeTransactionStatus(String),
    NewPendingTx(),
    UpdateTransactions(Vec<(String, Transaction, i64)>),
    BlockFound(Block),
    HeaderFound(BlockHeader, Height),
    POIResult(String),
    NotFound,
}

/// Envia un evento a la interfaz
pub fn send_event_to_ui(ui_sender: &Option<glib::Sender<UIEvent>>, event: UIEvent) {
    if let Some(ui_sender) = ui_sender {
        ui_sender
            .send(event)
            .expect("Error al enviar el evento a la interfaz");
    }
}
