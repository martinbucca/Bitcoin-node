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

/// Recibe un sender para enviarle el sender que envia eventos a la UI y un sender para enviarle eventos al nodo
/// Crea la UI y la ejecuta
pub fn run_ui(ui_sender: Sender<glib::Sender<UIEvent>>, sender_to_node: Sender<WalletEvent>) {
    let app = Application::builder()
        .application_id("org.gtk-rs.bitcoin")
        .build();
    app.connect_activate(move |_| {
        build_ui(&ui_sender, &sender_to_node);
    });
    let args: Vec<String> = vec![]; // necessary to not use main program args
    app.run_with_args(&args);
}

/// Recibe un sender para enviarle el sender que envia eventos a la UI y un sender para enviarle eventos al nodo
/// Inicializa la UI, carga el archivo glade y conecta los callbacks de los botones. Envia el sender que envia eventos a la UI al nodo y
/// muestra la ventana inicial
fn build_ui(ui_sender: &Sender<glib::Sender<UIEvent>>, sender_to_node: &Sender<WalletEvent>) {
    if gtk::init().is_err() {
        println!("Failed to initialize GTK.");
        return;
    }
    let (tx, rx) = glib::MainContext::channel(Priority::default());
    // envio sender de eventos a la UI al thread del nodo
    ui_sender.send(tx).expect("could not send sender to client");
    let builder = gtk::Builder::from_string(GLADE_FILE);
    add_css_to_screen();
    let initial_window: Window = builder
        .object("initial-window")
        .expect("no se pudo cargar la ventana inicial");
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
