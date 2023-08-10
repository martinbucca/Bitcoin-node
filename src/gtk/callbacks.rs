use super::ui_functions::{
    disable_buttons_and_entries, get_buttons, get_entries, hex_string_to_bytes,
    show_dialog_message_pop_up,
};
use crate::wallet_event::WalletEvent;
use gtk::{prelude::*, Builder, Spinner};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::mpsc::{self, Sender},
    time::Duration,
};

/// Receives a builder and a sender to send events to the node.
/// Connects the callbacks of the buttons and dynamic elements of the UI.
pub fn connect_ui_callbacks(builder: &Builder, sender_to_node: &Sender<WalletEvent>) {
    start_button_clicked(builder, sender_to_node.clone());
    send_button_clicked(builder, sender_to_node.clone());
    sync_balance_labels(builder);
    sync_account_labels(builder);
    search_blocks_button_clicked(builder, sender_to_node.clone());
    search_headers_button_clicked(builder, sender_to_node.clone());
    login_button_clicked(builder, sender_to_node.clone());
    dropdown_accounts_changed(builder, sender_to_node.clone());
    close_main_window_on_exit(builder, sender_to_node.clone());
    change_loading_account_label_periodically(builder);
    search_tx_poi_button_clicked(builder, sender_to_node.clone());
}

/// Connects the callback of the start button. When the button is clicked, it sends a Start event to the node.
fn start_button_clicked(builder: &Builder, sender: mpsc::Sender<WalletEvent>) {
    let start_button: gtk::Button = builder
        .object("start-button")
        .expect("error trying to get start button");
    let ref_start_btn = start_button.clone();
    start_button.connect_clicked(move |_| {
        sender
            .send(WalletEvent::Start)
            .expect("error sending start event to node");
        ref_start_btn.set_visible(false);
    });
}

/// Syncs the balance labels of the Overview and Send tabs so that they show the same balance always.
fn sync_balance_labels(builder: &Builder) {
    let available_label: gtk::Label = builder
        .object("available label")
        .expect("error trying to get the balance label");
    let send_balance: gtk::Label = builder
        .object("send-balance")
        .expect("error trying to get the balance label");
    let ref_to_available_label = available_label;
    // when one changes, the other changes automatically
    ref_to_available_label.connect_notify_local(Some("label"), move |label, _| {
        let new_text = label.text().to_string();
        send_balance.set_label(new_text.as_str());
    });
}

/// Syncs the account labels of the Overview and Send tabs so that they show the same account always.
fn sync_account_labels(builder: &Builder) {
    let account_login: gtk::Label = builder
        .object("status-login")
        .expect("error trying to get the account label");
    let overview_account: gtk::Label = builder
        .object("overview-account")
        .expect("error trying to get the account label");
    let ref_to_account = account_login;
    // when one changes, the other changes automatically
    ref_to_account.connect_notify_local(Some("label"), move |label, _| {
        let new_text = label.text().to_string();
        overview_account.set_label(new_text.as_str());
    });
}

/// Connects the callback of the send button. When the button is clicked, it sends a MakeTransaction event to the node.
/// In case the amount and fee are valid, it shows a pop up with the transaction information. Otherwise, it shows an error message.
fn send_button_clicked(builder: &Builder, sender: mpsc::Sender<WalletEvent>) {
    let send_button: gtk::Button = builder
        .object("send-button")
        .expect("error trying to get send button");
    let pay_to_entry: gtk::Entry = builder
        .object("pay to entry")
        .expect("error trying to get pay to entry");
    let fee_entry: gtk::Entry = builder
        .object("fee")
        .expect("error trying to get fee entry");
    let amount_entry: gtk::Entry = builder
        .object("amount-entry")
        .expect("error trying to get amount entry");
    send_button.connect_clicked(move |_| {
        let address_to_send = String::from(pay_to_entry.text());
        let amount = String::from(amount_entry.text());
        let fee: String = String::from(fee_entry.text());
        pay_to_entry.set_text("");
        amount_entry.set_text("");
        fee_entry.set_text("");
        if let Some((valid_amount, valid_fee)) = validate_amount_and_fee(amount, fee) {
            sender
                .send(WalletEvent::MakeTransaction(
                    address_to_send,
                    valid_amount,
                    valid_fee,
                ))
                .expect("error sending make transaction event to node");
        }
    });
}

/// Connects the callback of the search blocks button. When the button is clicked, it sends a SearchBlock event to the node.
/// In case the hash is valid, it shows a pop up with the block information. Otherwise, it shows an error message.
fn search_blocks_button_clicked(builder: &Builder, sender: mpsc::Sender<WalletEvent>) {
    let search_blocks_entry: gtk::SearchEntry = builder
        .object("search-block")
        .expect("error trying to get search blocks entry");
    let search_blocks_button: gtk::Button = builder
        .object("search-blocks-button")
        .expect("error trying to get search blocks button");
    search_blocks_button.connect_clicked(move |_| {
        let text = search_blocks_entry.text().to_string();
        if let Some(block_hash) = hex_string_to_bytes(text.as_str()) {
            sender
                .send(WalletEvent::SearchBlock(block_hash))
                .expect("Error sending search block event to node");
        } else {
            show_dialog_message_pop_up(
                format!("Error {text} is not a valid block hash").as_str(),
                "Error searching block",
            )
        }
        search_blocks_entry.set_text("");
    });
}

/// Connects the callback of the search headers button. When the button is clicked, it sends a SearchHeader event 
/// to the node. In case the hash is valid, it shows a pop up with the header information. 
/// Otherwise, it shows an error message.
fn search_headers_button_clicked(builder: &Builder, sender: mpsc::Sender<WalletEvent>) {
    let search_headers_entry: gtk::SearchEntry = builder
        .object("search-block-headers")
        .expect("error trying to get search headers entry");
    let search_headers_button: gtk::Button = builder
        .object("search-header-button")
        .expect("error trying to get search headers button");
    search_headers_button.connect_clicked(move |_| {
        let text = search_headers_entry.text().to_string();
        if let Some(block_hash) = hex_string_to_bytes(text.as_str()) {
            sender.send(WalletEvent::SearchHeader(block_hash)).expect("Error sending search header event to node");
        } else {
            show_dialog_message_pop_up(
                format!("Error {text} is not a valid block hash").as_str(),
                "Error searching header",
            )
        }
        search_headers_entry.set_text("");
    });
}

/// Connects the callback of the login button. When the button is clicked, it sends an AddAccountRequest 
/// event to the node.
fn login_button_clicked(builder: &Builder, sender: mpsc::Sender<WalletEvent>) {
    // elementos de la interfaz
    let login_button: gtk::Button = builder
        .object("login-button")
        .expect("error trying to get login button");
    let address_entry: gtk::Entry = builder
        .object("address")
        .expect("error trying to get address entry");
    let private_key_entry: gtk::Entry = builder
        .object("private-key")
        .expect("error trying to get private key entry");
    let account_loading_spinner: Spinner = builder
        .object("account-spin")
        .expect("error trying to get account loading spinner");
    let loading_account_label: gtk::Label = builder
        .object("load-account")
        .expect("error trying to get loading account label");
    let ref_account_spin = account_loading_spinner;
    let ref_loading_account_label = loading_account_label;
    let dropdown: gtk::ComboBoxText = builder
        .object("dropdown-menu")
        .expect("error trying to get dropdown menu");
    let ref_to_dropdown = dropdown;
    let ref_to_buttons = get_buttons(builder);
    let ref_to_entries = get_entries(builder);
    // action when login button is clicked
    login_button.connect_clicked(move |_| {
        disable_buttons_and_entries(&ref_to_buttons, &ref_to_entries);
        ref_to_dropdown.set_sensitive(false);
        ref_account_spin.set_visible(true);
        ref_loading_account_label.set_visible(true);
        let address = String::from(address_entry.text());
        let private_key = String::from(private_key_entry.text());
        address_entry.set_text("");
        private_key_entry.set_text("");
        sender
            .send(WalletEvent::AddAccountRequest(private_key, address))
            .expect("error sending add account request event to node");
    });
}

/// Connects the callback of the dropdown menu. When the dropdown menu is changed, 
/// it sends a ChangeAccount event to the node.
fn dropdown_accounts_changed(builder: &Builder, sender: mpsc::Sender<WalletEvent>) {
    let dropdown: gtk::ComboBoxText = builder
        .object("dropdown-menu")
        .expect("error trying to get dropdown menu");
    let status_login: gtk::Label = builder
        .object("status-login")
        .expect("error trying to get status login label");
    dropdown.connect_changed(move |combobox| {
        // get the text of the selected option
        if let Some(selected_text) = combobox.active_text() {
            status_login.set_label(selected_text.as_str());
            status_login.set_visible(true);
            if let Some(new_index) = combobox.active() {
                sender
                    .send(WalletEvent::ChangeAccount(new_index as usize))
                    .expect("Error sending change account event to node");
            }
        }
    });
}

/// Connects the callback of the main window. When the main window is closed, it sends a Finish event to the node.
fn close_main_window_on_exit(builder: &Builder, sender: mpsc::Sender<WalletEvent>) {
    let main_window: gtk::Window = builder
        .object("main-window")
        .expect("error trying to get main window");
    main_window.connect_delete_event(move |_, _| {
        sender
            .send(WalletEvent::Finish)
            .expect("Error sending finish event to node");
        gtk::main_quit();
        Inhibit(false)
    });
}

/// Changes the loading account label every 5 seconds
fn change_loading_account_label_periodically(builder: &Builder) {
    let loading_account_label: gtk::Label = builder
        .object("load-account")
        .expect("error trying to get loading account label");
    let ref_to_loading_account_label = Rc::new(RefCell::new(loading_account_label));
    gtk::glib::timeout_add_local(Duration::from_secs(5), move || {
        update_label(ref_to_loading_account_label.clone());
        Continue(true)
    });
}

/// Connects the callback of the search tx poi button. When the button is clicked, it sends a PoiOfTransactionRequest
pub fn search_tx_poi_button_clicked(builder: &Builder, sender: mpsc::Sender<WalletEvent>) {
    let search_poi_tx_entry: gtk::Entry = builder
        .object("search-tx")
        .expect("error trying to get the entry of search tx in callback");
    let search_poi_block_entry: gtk::Entry = builder
        .object("search-poi-block")
        .expect("error trying to get the entry of search block in callback");
    let search_tx_poi_button: gtk::Button = builder
        .object("search-tx-button")
        .expect("error trying to get the button of search tx in callback");

    search_tx_poi_button.connect_clicked(move |_| {
        let tx_hash_string = search_poi_tx_entry.text().to_string();
        let block_hash_string = search_poi_block_entry.text().to_string();
        let mut tx_is_valid = true;
        let mut block_is_valid = true;
        if hex_string_to_bytes(tx_hash_string.as_str()).is_none() {
            tx_is_valid = false;
        }
        if hex_string_to_bytes(tx_hash_string.as_str()).is_none() {
            block_is_valid = false;
        }

        if tx_is_valid && block_is_valid {
            sender
                .send(WalletEvent::PoiOfTransactionRequest(
                    block_hash_string,
                    tx_hash_string,
                ))
                .expect("Error sending poi of transaction request event to node");
        } else if !tx_is_valid && !block_is_valid {
            show_dialog_message_pop_up(
                format!("Error {tx_hash_string} and {block_hash_string} are not valid hashes")
                    .as_str(),
                "Error searching POI",
            )
        } else if !tx_is_valid {
            show_dialog_message_pop_up(
                format!("Error {tx_hash_string} is not a valid tx hash").as_str(),
                "Error searching tx",
            )
        } else if !block_is_valid {
            show_dialog_message_pop_up(
                format!("Error {block_hash_string} is not a valid block hash").as_str(),
                "Error searching block",
            )
        }
        search_poi_tx_entry.set_text("");
        search_poi_block_entry.set_text("");
    });
}

/*
***************************************************************************
************************ AUXILIAR FUNCTIONS *******************************
***************************************************************************
*/

/// Checks if the user entered a valid amount and fee. In case it is not, it shows a pop up with an error message.
fn validate_amount_and_fee(amount: String, fee: String) -> Option<(i64, i64)> {
    let valid_amount = match amount.parse::<i64>() {
        Ok(amount) => amount,
        Err(_) => {
            show_dialog_message_pop_up(
                "Error, please enter a valid amount of Satoshis",
                "Failed to make transaction",
            );
            return None;
        }
    };
    let valid_fee = match fee.parse::<i64>() {
        Ok(fee) => fee,
        Err(_) => {
            show_dialog_message_pop_up(
                "Error, please enter a valid fee of Satoshis",
                "Failed to make transaction",
            );
            return None;
        }
    };

    Some((valid_amount, valid_fee))
}

/// Receives a Label and changes its text to the next one in the waiting_labels list.
fn update_label(label: Rc<RefCell<gtk::Label>>) -> Continue {
    let waiting_labels = [
        "Hold tight! Setting up your Bitcoin account...",
        "We're ensuring your account's security...",
        "Be patient! Your Bitcoin account is being created...",
    ];
    let current_text = label.borrow().text().to_string();
    for i in 0..waiting_labels.len() {
        if current_text == waiting_labels[i] {
            let next_text = waiting_labels[(i + 1) % waiting_labels.len()];
            label.borrow().set_text(next_text);
            break;
        }
    }
    Continue(true)
}
