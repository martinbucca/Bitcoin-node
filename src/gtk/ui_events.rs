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
/// Represents the events that the wallet sends to the UI to display
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
    UpdateHeadersDownloaded(usize),
    UpdateBlocksDownloaded(usize, usize),
    MakeTransactionStatus(String),
    NewPendingTx(),
    UpdateTransactions(Vec<(String, Transaction, i64)>),
    BlockFound(Block),
    HeaderFound(BlockHeader, Height),
    POIResult(String),
    NotFound,
}

/// Sends an event to the UI
pub fn send_event_to_ui(ui_sender: &Option<glib::Sender<UIEvent>>, event: UIEvent) {
    if let Some(ui_sender) = ui_sender {
        ui_sender
            .send(event)
            .expect("Error trying to send event to UI");
    }
}
