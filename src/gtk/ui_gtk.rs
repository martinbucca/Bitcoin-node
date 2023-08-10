use std::sync::mpsc::Sender;

use super::ui_events::UIEvent;
use super::ui_functions::set_icon;
use super::{
    callbacks::connect_ui_callbacks,
    ui_functions::{add_css_to_screen, handle_ui_event},
};
use crate::wallet_event::WalletEvent;
use gtk::{
    glib::{self, Priority},
    prelude::*,
    Application, Window,
};

const GLADE_FILE: &str = include_str!("resources/interfaz.glade");

/// Receives a sender to send the sender that sends events to the UI and a sender to send events to the node.
/// Creates the UI and runs it
pub fn run_ui(ui_sender: Sender<glib::Sender<UIEvent>>, sender_to_node: Sender<WalletEvent>) {
    let app = Application::builder()
        .application_id("org.gtk-rs.bitcoin")
        .build();
    app.connect_activate(move |_| {
        build_ui(&ui_sender, &sender_to_node);
    });
    let args: Vec<String> = vec![]; // necessary not to use main program args
    app.run_with_args(&args);
}

/// Receives a sender to send the sender that sends events to the UI and a sender to send events to the node.
/// Initializes the UI, loads the glade file and connects the callbacks of the buttons. 
/// Sends the sender that sends events to the UI to the node and shows the initial window.
fn build_ui(ui_sender: &Sender<glib::Sender<UIEvent>>, sender_to_node: &Sender<WalletEvent>) {
    if gtk::init().is_err() {
        println!("Failed to initialize GTK.");
        return;
    }
    let (tx, rx) = glib::MainContext::channel(Priority::default());
    // send sender of events to the UI to the node thread
    ui_sender.send(tx).expect("could not send sender to client");
    let builder = gtk::Builder::from_string(GLADE_FILE);
    add_css_to_screen();
    let initial_window: Window = builder
        .object("initial-window")
        .expect("initial window not found");
    initial_window.set_title("Bitcoin Wallet");
    set_icon(&initial_window);
    initial_window.show();
    let tx_to_node = sender_to_node.clone();
    let builder_clone = builder.clone();
    rx.attach(None, move |msg| {
        handle_ui_event(builder_clone.clone(), msg, tx_to_node.clone());
        Continue(true)
    });
    connect_ui_callbacks(&builder, sender_to_node);
    gtk::main();
}
