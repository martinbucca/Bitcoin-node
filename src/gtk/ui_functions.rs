use std::{
    collections::HashMap,
    sync::{
        mpsc::{self},
        Arc, RwLock,
    },
};

use gtk::{
    gdk,
    gdk_pixbuf::{self, Pixbuf},
    prelude::*,
    Builder, CssProvider, ProgressBar, Spinner, StyleContext, TreeView, Window,
};

use crate::{
    account::Account,
    blocks::{block::Block, block_header::BlockHeader},
    transactions::transaction::Transaction,
    wallet_event::WalletEvent,
};

use super::ui_events::UIEvent;

type Blocks = Arc<RwLock<HashMap<[u8; 32], Block>>>;
type Headers = Arc<RwLock<Vec<BlockHeader>>>;

const AMOUNT_TO_SHOW: usize = 500;
const ICON_FILE: &str = "src/gtk/resources/icon.png";


/// Handles each event received from the wallet. Decide what to do with each event.
pub fn handle_ui_event(
    builder: Builder,
    ui_event: UIEvent,
    sender_to_node: mpsc::Sender<WalletEvent>,
) {
    let tx_table: TreeView = builder
        .object("tx_table")
        .expect("Error trying to get the table of transactions");
    match ui_event {
        UIEvent::UpdateBlocksDownloaded(blocks_downloaded, blocks_to_download) => {
            update_progress_bar(&builder, blocks_downloaded, blocks_to_download);
        }
        UIEvent::StartHandshake => {
            update_message_header(&builder, "Making handshake with nodes...");
        }
        UIEvent::UpdateHeadersDownloaded(headers_downloaded) => {
            update_message_header(
                &builder,
                format!("Headers downloaded: {}", headers_downloaded).as_str(),
            );
        }
        UIEvent::InitializeUITabs((headers, blocks)) => {
            render_main_window(&builder, &headers, &blocks);
        }
        UIEvent::StartDownloadingHeaders => {
            let message_header: gtk::Label = builder
                .object("message-header")
                .expect("Error trying to get the message header label");
            let spinner: Spinner = builder
                .object("header-spin")
                .expect("Error trying to get the header spinner");
            let headers_box: gtk::Box = builder
                .object("headers-box")
                .expect("Error trying to get the headers box");
            headers_box.set_visible(true);
            message_header.set_visible(true);
            spinner.set_visible(true);
        }
        UIEvent::FinsihDownloadingHeaders(headers) => {
            update_message_and_spinner(
                &builder,
                false,
                format!("TOTAL HEADERS DOWNLOADED: {}", headers).as_str(),
            );
        }
        UIEvent::StartDownloadingBlocks => {
            render_progress_bar(&builder);
        }
        UIEvent::AccountAddedSuccesfully(account) => {
            update_account_tab(&builder, account);
        }
        UIEvent::AddAccountError(error) => {
            render_account_tab(&builder);
            show_dialog_message_pop_up(error.as_str(), "Error trying to add account");
        }
        UIEvent::ChangeAccountError(error) => {
            show_dialog_message_pop_up(error.as_str(), "Error trying to change account");
        }
        UIEvent::AccountChanged(account) => {
            println!("Account changed to: {}", account.address);
            let available_label = builder
                .object("available label")
                .expect("Error trying to get the available label");
            update_overview(&account, &available_label, &builder);
            // updates the transactions tab
            sender_to_node
                .send(WalletEvent::GetTransactionsRequest)
                .expect(
                    "Error sending get transactions request after changing account in ui_events",
                );
        }
        UIEvent::MakeTransactionStatus(status) => {
            show_dialog_message_pop_up(status.as_str(), "transaction's status");
        }
        UIEvent::AddBlock(block) => {
            handle_add_block(sender_to_node, &builder, &block);
        }
        UIEvent::ShowPendingTransaction(account, transaction) => {
            show_dialog_message_pop_up(
                format!(
                    "New incoming pending transaction: {} received for account: {}",
                    transaction.hex_hash(),
                    account.address
                )
                .as_str(),
                "Account added succesfully",
            );
            sender_to_node
                .send(WalletEvent::GetTransactionsRequest)
                .expect("Error sending get transactions request");
        }

        UIEvent::UpdateTransactions(transactions) => {
            render_transactions(&transactions, tx_table);
            render_recent_transactions(&transactions, &builder);
        }

        UIEvent::NewPendingTx() => {
            sender_to_node
                .send(WalletEvent::GetTransactionsRequest)
                .expect("Error sending get transactions request");
        }
        UIEvent::ShowConfirmedTransaction(block, account, transaction) => {
            show_dialog_message_pop_up(
                format!(
                    "Transaction confirmed: {} for account: {} in block: {}",
                    transaction.hex_hash(),
                    account.address,
                    block.hex_hash()
                )
                .as_str(),
                "Account added succesfully",
            );
            sender_to_node
                .send(WalletEvent::GetTransactionsRequest)
                .expect("Error sending get transactions request");
        }
        UIEvent::BlockFound(block) => {
            show_dialog_message_pop_up(
                format!(
                    "Height: {} \nHash: {} \nTime (UTC): {} \nTx Count: {}",
                    block.get_height(),
                    block.hex_hash(),
                    block.local_time(),
                    block.txn_count.decoded_value()
                )
                .as_str(),
                "Block found",
            );
        }
        UIEvent::HeaderFound(header, height) => {
            show_dialog_message_pop_up(
                format!(
                    "Height: {} \nHash: {} \nTime: {}",
                    height,
                    header.hex_hash(),
                    header.local_time()
                )
                .as_str(),
                "Header found",
            );
        }
        UIEvent::NotFound => {
            show_dialog_message_pop_up("Not found", "Not found");
        }
        UIEvent::POIResult(message) => {
            show_dialog_message_pop_up(message.as_str(), "POI Result");
        }
    }
}

/// Shows the transactions in the transactions tab.
fn render_transactions(transactions: &Vec<(String, Transaction, i64)>, tx_table: TreeView) {
    let tree_model = gtk::ListStore::new(&[
        gdk_pixbuf::Pixbuf::static_type(),
        String::static_type(),
        String::static_type(),
        String::static_type(),
        i32::static_type(),
    ]);

    for tx in transactions {
        let status_icon_pixbuf = if tx.0 == "Pending" {
            // Load the image "Pending.png" and convert it to a GdkPixbuf
            Pixbuf::from_file("src/gtk/resources/pending.png").ok()
        } else {
            // Load the image "Confirmed.png" and convert it to a GdkPixbuf
            Pixbuf::from_file("src/gtk/resources/confirmed.png").ok()
        };

        let row = tree_model.append();
        if let Some(pixbuf) = status_icon_pixbuf {
            tree_model.set(
                &row,
                &[
                    (0, &pixbuf.to_value()),
                    (1, &tx.0.to_value()),
                    (2, &tx.1.hex_hash().to_value()),
                    (3, &"P2PKH".to_value()),
                    (4, &tx.2.to_value()),
                ],
            );
        }
    }
    tx_table.set_model(Some(&tree_model));
}

/// Shows the recent transactions in the overview tab.
fn render_recent_transactions(transactions: &Vec<(String, Transaction, i64)>, builder: &Builder) {
    // Get the last five elements or all elements if there are fewer than five
    let recent_transactions = if transactions.len() <= 5 {
        &transactions[..]
    } else {
        &transactions[transactions.len() - 5..]
    };
    let amount_labels = [
        "amount-tx-1",
        "amount-tx-2",
        "amount-tx-3",
        "amount-tx-4",
        "amount-tx-5",
    ];
    let icons = [
        "icon-tx-1",
        "icon-tx-2",
        "icon-tx-3",
        "icon-tx-4",
        "icon-tx-5",
    ];
    let type_labels = [
        "type-tx-1",
        "type-tx-2",
        "type-tx-3",
        "type-tx-4",
        "type-tx-5",
    ];
    let recent_tx = [
        "recent-tx-1",
        "recent-tx-2",
        "recent-tx-3",
        "recent-tx-4",
        "recent-tx-5",
    ];
    for (i, tx) in recent_transactions.iter().enumerate() {
        let hash: gtk::AccelLabel = builder
            .object(recent_tx[i])
            .expect("error trying to get the recent transaction");
        hash.set_label(&tx.1.hex_hash());
        hash.set_visible(true);
        let amount_label: gtk::AccelLabel = builder
            .object(amount_labels[i])
            .expect("error trying to get the amount label of the recent transaction");
        amount_label.set_label(format!("{} Satoshis", tx.2).as_str());
        amount_label.set_visible(true);
        let icon: gtk::Image = builder
            .object(icons[i])
            .expect("error trying to get the icon of the recent transaction");
        if tx.0 == "Pending" {
            icon.set_from_file(Some("src/gtk/resources/ov_pending.png"));
        } else {
            icon.set_from_file(Some("src/gtk/resources/ov_confirmed.png"));
        }
        icon.set_visible(true);
        let type_label: gtk::AccelLabel = builder
            .object(type_labels[i])
            .expect("error trying to get the type label of the recent transaction");
        type_label.set_visible(true);
    }
}

/// Adds a block and header to the tabs.
/// Asks the wallet for the account to update the information.
fn handle_add_block(sender_to_node: mpsc::Sender<WalletEvent>, builder: &Builder, block: &Block) {
    let liststore_blocks: gtk::ListStore = builder
        .object("liststore-blocks")
        .expect("Error trying to get the liststore of blocks");
    let liststore_headers: gtk::ListStore = builder
        .object("liststore-headers")
        .expect("Error trying to get the liststore of headers");

    add_row_first_to_liststore_block(&liststore_blocks, block);
    add_row_first_to_liststore_headers(&liststore_headers, &block.block_header, block.get_height());

    sender_to_node
        .send(WalletEvent::GetAccountRequest)
        .expect("Error sending get account request");
}

/// Updates and shows the progress bar representing the blocks download, in the initial window.
fn update_progress_bar(builder: &Builder, blocks_downloaded: usize, blocks_to_download: usize) {
    let progress_bar: ProgressBar = builder
        .object("block-bar")
        .expect("Error trying to get the progress bar");
    progress_bar.set_fraction(blocks_downloaded as f64 / blocks_to_download as f64);
    progress_bar.set_text(Some(
        format!(
            "Blocks downloaded: {}/{}",
            blocks_downloaded, blocks_to_download
        )
        .as_str(),
    ));
}

/// Updates the amount of headers shown in the initial window.
fn update_message_header(builder: &Builder, msg: &str) {
    let message_header: gtk::Label = builder
        .object("message-header")
        .expect("Error trying to get the message header");
    message_header.set_label(msg);
}

/// Shows the total headers downloaded in the initial window. Hinds the spinner and the message header.
fn update_message_and_spinner(builder: &Builder, visible: bool, msg: &str) {
    let total_headers_label: gtk::Label = builder
        .object("total-headers")
        .expect("Error trying to get the total headers label");
    let total_headers_box: gtk::Box = builder
        .object("total-box")
        .expect("Error trying to get the total box");
    let message_header: gtk::Label = builder
        .object("message-header")
        .expect("Error trying to get the message header");
    let headers_box: gtk::Box = builder
        .object("headers-box")
        .expect("Error trying to get the headers box");
    let spinner: Spinner = builder
        .object("header-spin")
        .expect("Error trying to get the header spinner");
    message_header.set_visible(visible);
    spinner.set_visible(visible);
    headers_box.set_visible(visible);
    total_headers_box.set_visible(true);
    total_headers_label.set_label(msg);
    total_headers_label.set_visible(true);
}

/// Shows the progess bar in the initial window.
fn render_progress_bar(builder: &Builder) {
    let progress_bar: ProgressBar = builder
        .object("block-bar")
        .expect("Error trying to get the progress bar");
    progress_bar.set_visible(true);
    progress_bar.set_text(Some("Blocks downloaded: 0"));
}

/// Closes the initial window and initializes tand shows the main window.
fn render_main_window(builder: &Builder, headers: &Headers, blocks: &Blocks) {
    let initial_window: gtk::Window = builder
        .object("initial-window")
        .expect("Error trying to get the initial window");
    let main_window: gtk::Window = builder
        .object("main-window")
        .expect("Error trying to get the main window");
    let liststore_blocks: gtk::ListStore = builder
        .object("liststore-blocks")
        .expect("Error trying to get the liststore of blocks");
    let liststore_headers: gtk::ListStore = builder
        .object("liststore-headers")
        .expect("Error trying to get the liststore of headers");
    let header_table: TreeView = builder
        .object("header_table")
        .expect("Error trying to get the table of headers");
    let block_table: TreeView = builder
        .object("block_table")
        .expect("Error trying to get the table of blocks");
    initial_window.close();
    main_window.set_title("Bitcoin Wallet");
    set_icon(&main_window);
    main_window.show();
    initialize_headers_tab(&liststore_headers, &header_table, headers);
    initialize_blocks_tab(&liststore_blocks, &block_table, headers, blocks);
}

/// Updates the account tab with the account received.
fn update_account_tab(builder: &Builder, account: Account) {
    let account_loading_spinner: Spinner = builder
        .object("account-spin")
        .expect("Error trying to get the account spinner");
    let loading_account_label: gtk::Label = builder
        .object("load-account")
        .expect("Error trying to get the account label");
    let dropdown: gtk::ComboBoxText = builder
        .object("dropdown-menu")
        .expect("Error trying to get the dropdown menu");
    account_loading_spinner.set_visible(false);
    loading_account_label.set_visible(false);
    let buttons = get_buttons(builder);
    let entries = get_entries(builder);
    enable_buttons_and_entries(&buttons, &entries);
    dropdown.set_sensitive(true);
    show_dialog_message_pop_up(
        format!("Account {} added to wallet!", account.address).as_str(),
        "Account added succesfully",
    );
    dropdown.append_text(account.address.as_str());
}

/// Shows the account tab.
fn render_account_tab(builder: &Builder) {
    let account_loading_spinner: Spinner = builder
        .object("account-spin")
        .expect("Error trying to get the account spinner");
    let loading_account_label: gtk::Label = builder
        .object("load-account")
        .expect("Error trying to get the account label");
    let dropdown: gtk::ComboBoxText = builder
        .object("dropdown-menu")
        .expect("Error trying to get the dropdown menu");
    let buttons = get_buttons(builder);
    let entries = get_entries(builder);
    enable_buttons_and_entries(&buttons, &entries);
    account_loading_spinner.set_visible(false);
    loading_account_label.set_visible(false);
    dropdown.set_sensitive(true);
}

/// Gets the buttons of the interface.
pub fn get_buttons(builder: &Builder) -> Vec<gtk::Button> {
    let buttons = vec![
        builder
            .object("send-button")
            .expect("Error trying to get the send button"),
        builder
            .object("search-tx-button")
            .expect("Error trying to get the search tx button"),
        builder
            .object("search-blocks-button")
            .expect("Error trying to get the search blocks button"),
        builder
            .object("search-header-button")
            .expect("Error trying to get the search header button"),
        builder
            .object("login-button")
            .expect("Error trying to get the login button"),
    ];
    buttons
}
/// Gets the entries of the interface.
pub fn get_entries(builder: &Builder) -> Vec<gtk::Entry> {
    let entries = vec![
        builder
            .object("pay to entry")
            .expect("Error trying to get the pay to entry"),
        builder
            .object("amount-entry")
            .expect("Error trying to get the amount entry"),
        builder
            .object("fee")
            .expect("Error trying to get the fee entry"),
        builder
            .object("search-tx")
            .expect("Error trying to get the search tx entry"),
        builder
            .object("search-block")
            .expect("Error trying to get the search block entry"),
        builder
            .object("search-block-headers")
            .expect("Error trying to get the search block headers entry"),
        builder
            .object("address")
            .expect("Error trying to get the address entry"),
        builder
            .object("private-key")
            .expect("Error trying to get the private key entry"),
    ];
    entries
}

/// Receives the liststore of blocks, a Treeview to show the blocks, the headers and the blocks.
/// Initializes the blocks tab with the blocks and headers received.
fn initialize_blocks_tab(
    liststore_blocks: &gtk::ListStore,
    block_table: &TreeView,
    headers: &Headers,
    blocks: &Blocks,
) {
    // temporal tree model
    let tree_model = gtk::ListStore::new(&[
        String::static_type(),
        String::static_type(),
        String::static_type(),
    ]);
    block_table.set_model(Some(&tree_model));
    let mut block_hash: Vec<[u8; 32]> = Vec::new();
    for header in headers.read().unwrap().iter().rev().take(AMOUNT_TO_SHOW) {
        block_hash.push(header.hash());
    }

    for hash in block_hash {
        let blocks_lock = blocks.read().unwrap();
        let block = blocks_lock.get(&hash).unwrap();
        add_row_last_to_liststore_block(liststore_blocks, block)
    }
    block_table.set_model(Some(liststore_blocks));
}

fn initialize_headers_tab(
    liststore_headers: &gtk::ListStore,
    header_table: &TreeView,
    headers: &Headers,
) {
    // temporal tree model
    let tree_model = gtk::ListStore::new(&[
        String::static_type(),
        String::static_type(),
        String::static_type(),
    ]);
    header_table.set_model(Some(&tree_model));

    for (index, header) in headers
        .read()
        .unwrap()
        .iter()
        .enumerate()
        .rev()
        .take(AMOUNT_TO_SHOW / 2)
    {
        add_row_last_to_liststore_headers(liststore_headers, header, index as u32);
    }

    for (index, header) in headers
        .read()
        .unwrap()
        .iter()
        .enumerate()
        .skip(1) // Skip first header
        .take(AMOUNT_TO_SHOW / 2)
        .rev()
    {
        add_row_last_to_liststore_headers(liststore_headers, header, index as u32);
    }

    header_table.set_model(Some(liststore_headers));
}

/// Adds a row to the liststore of blocks.
fn add_row_last_to_liststore_block(liststore_blocks: &gtk::ListStore, block: &Block) {
    let row = liststore_blocks.append();
    add_block_row(liststore_blocks, row, block);
}

/// Adds a row to the liststore of blocks.
fn add_row_first_to_liststore_block(liststore_blocks: &gtk::ListStore, block: &Block) {
    let row = liststore_blocks.prepend();
    add_block_row(liststore_blocks, row, block);
}
/// Adds a row to the liststore of blocks.
fn add_block_row(liststore_blocks: &gtk::ListStore, row: gtk::TreeIter, block: &Block) {
    liststore_blocks.set(
        &row,
        &[
            (0, &block.get_height().to_value()),
            (1, &block.hex_hash()),
            (2, &block.local_time()),
            (3, &block.txn_count.decoded_value().to_value()),
        ],
    );
}

/// Adds a row to the liststore of headers.
fn add_row_last_to_liststore_headers(
    liststore_headers: &gtk::ListStore,
    header: &BlockHeader,
    height: u32,
) {
    let row = liststore_headers.append();
    add_header_row(liststore_headers, row, header, height);
}

/// Adds a row to the liststore of headers.
fn add_row_first_to_liststore_headers(
    liststore_headers: &gtk::ListStore,
    header: &BlockHeader,
    height: u32,
) {
    let row = liststore_headers.prepend();
    add_header_row(liststore_headers, row, header, height);
}
/// Adds a row to the liststore of headers.
fn add_header_row(
    liststore_headers: &gtk::ListStore,
    row: gtk::TreeIter,
    header: &BlockHeader,
    height: u32,
) {
    liststore_headers.set(
        &row,
        &[
            (0, &height.to_value()),
            (1, &header.hex_hash()),
            (2, &header.local_time()),
        ],
    );
}

/// Updates the overview tab with the account information.
fn update_overview(account: &Account, available_label: &gtk::Label, builder: &Builder) {
    available_label.set_label(format!("{}", account.balance()).as_str());
    let amount_labels = [
        "amount-tx-1",
        "amount-tx-2",
        "amount-tx-3",
        "amount-tx-4",
        "amount-tx-5",
    ];
    let icons = [
        "icon-tx-1",
        "icon-tx-2",
        "icon-tx-3",
        "icon-tx-4",
        "icon-tx-5",
    ];
    let type_labels = [
        "type-tx-1",
        "type-tx-2",
        "type-tx-3",
        "type-tx-4",
        "type-tx-5",
    ];
    let recent_tx = [
        "recent-tx-1",
        "recent-tx-2",
        "recent-tx-3",
        "recent-tx-4",
        "recent-tx-5",
    ];
    for i in 0..5 {
        let hash: gtk::AccelLabel = builder
            .object(recent_tx[i])
            .expect("Error trying to get hash label");
        hash.set_visible(false);
        let amount_label: gtk::AccelLabel = builder
            .object(amount_labels[i])
            .expect("Error trying to get amount label");
        amount_label.set_visible(false);
        let icon: gtk::Image = builder
            .object(icons[i])
            .expect("Error trying to get icon");
        icon.set_visible(false);
        let type_label: gtk::AccelLabel = builder
            .object(type_labels[i])
            .expect("Error trying to get type label");
        type_label.set_visible(false);
    }
}

/// Enables the buttons and entries of the UI.
pub fn enable_buttons_and_entries(buttons: &Vec<gtk::Button>, entries: &Vec<gtk::Entry>) {
    for button in buttons {
        button.set_sensitive(true);
    }
    for entry in entries {
        entry.set_sensitive(true);
    }
}

/// Disables the buttons and entries of the UI.
pub fn disable_buttons_and_entries(buttons: &Vec<gtk::Button>, entries: &Vec<gtk::Entry>) {
    for button in buttons {
        button.set_sensitive(false);
    }
    for entry in entries {
        entry.set_sensitive(false);
    }
}

/// Shows a pop up dialog with a message.
pub fn show_dialog_message_pop_up(message: &str, title: &str) {
    let dialog = gtk::MessageDialog::new(
        None::<&Window>,
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Info,
        gtk::ButtonsType::Ok,
        message,
    );
    dialog.set_title(title);
    dialog.set_keep_above(true);
    let content_area = dialog.content_area();
    content_area.style_context().add_class("dialog");
    dialog.run();
    dialog.close();
}

/// Converts a hexadecimal string to a byte array representing the hash.
/// Receives a hexadecimal string of 64 characters.
/// Returns an array of 32 bytes.
/// If the string is not hexadecimal or does not have 64 characters, returns None.
pub fn hex_string_to_bytes(hex_string: &str) -> Option<[u8; 32]> {
    if hex_string.len() != 64 {
        return None; // The length of the string is not 64
    }
    let mut result = [0u8; 32];
    let hex_chars: Vec<_> = hex_string.chars().collect();
    for i in 0..32 {
        let start = i * 2;
        let end = start + 2;
        if let Ok(byte) = u8::from_str_radix(&hex_chars[start..end].iter().collect::<String>(), 16)
        {
            result[31 - i] = byte; // Invert the order of the bytes
        } else {
            return None; // The string is not hexadecimal
        }
    }
    Some(result)
}

/// Adds the style of the css file to the screen.
pub fn add_css_to_screen() {
    let css_provider: CssProvider = CssProvider::new();
    css_provider
        .load_from_path("src/gtk/resources/styles.css")
        .expect("Failed to load CSS file.");
    let screen: gdk::Screen = gdk::Screen::default().expect("Failed to get default screen.");
    StyleContext::add_provider_for_screen(
        &screen,
        &css_provider,
        gtk::STYLE_PROVIDER_PRIORITY_USER,
    );
}

/// Sets the icon to the app.
pub fn set_icon(window: &gtk::Window) {
    if let Ok(icon_pixbuf) = Pixbuf::from_file(ICON_FILE) {
            if let Some(icon) = icon_pixbuf.scale_simple(64, 64, gdk_pixbuf::InterpType::Bilinear) {
                window.set_icon(Some(&icon));
                }
    }
}