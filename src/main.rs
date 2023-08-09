use bitcoin::blockchain_download::initial_block_download;
use bitcoin::config::Config;
use bitcoin::custom_errors::NodeCustomErrors;
use bitcoin::gtk::ui_events::{send_event_to_ui, UIEvent};
use bitcoin::gtk::ui_gtk::run_ui;
use bitcoin::handshake::handshake_with_nodes;
use bitcoin::logwriter::log_writer::{
    set_up_loggers, shutdown_loggers, LogSender, LogSenderHandles,
};
use bitcoin::network::get_active_nodes_from_dns_seed;
use bitcoin::node::Node;
use bitcoin::server::NodeServer;
use bitcoin::terminal_ui::terminal_ui;
use bitcoin::wallet::Wallet;
use bitcoin::wallet_event::{handle_ui_request, WalletEvent};
use gtk::glib;
use std::sync::mpsc::{channel, Receiver};
use std::{env, thread};

/// Recibe los argumentos del programa y corre el nodo con o sin interfaz grafica segun los argumentos
/// Si recibe 3 argumentos y el ultimo es -i corre el nodo con interfaz grafica
/// Devuelve un error si no se puede correr el nodo correctamente o si no se puede crear la interfaz grafica
/// Ok(()) si se corre el nodo correctamente
fn main() -> Result<(), NodeCustomErrors> {
    let mut args: Vec<String> = env::args().collect();
    if args.len() == 3 && args[2] == *"-i" {
        // pop the last argument (-i)
        args.pop();
        run_with_ui(args)?;
    } else {
        run_without_ui(&args)?;
    }
    Ok(())
}

/// Crea los channels para comunicar el nodo con la interfaz grafica, corre
/// la interfaz grafica en el thread principal y corre el nodo en un thread secundario
/// Devuelve un error si no se puede crear la interfaz grafica o si no se puede correr el nodo
/// Ok(()) si se corre el nodo correctamente
fn run_with_ui(args: Vec<String>) -> Result<(), NodeCustomErrors> {
    // Channel created to recibe the sender from the ui (channel created in the ui thread) that is needed to send events to the ui
    let (tx, rx) = channel();
    // Channel to comunicate the ui with the node
    let (sender_from_ui_to_node, receiver_from_ui_to_node) = channel();
    let app_thread = thread::spawn(move || -> Result<(), NodeCustomErrors> {
        // Recieve the sender from the ui thread to send events to the ui
        let ui_tx = rx.recv().map_err(|err| {
            println!("ERROR AL RECIBIR!");
            NodeCustomErrors::ThreadChannelError(err.to_string())
        })?;
        // run the node with the ui sender
        run_node(&args, Some(ui_tx), Some(receiver_from_ui_to_node))
    });
    // run the ui in the main thread
    run_ui(tx, sender_from_ui_to_node);
    app_thread
        .join()
        .map_err(|err| NodeCustomErrors::ThreadJoinError(format!("{:?}", err)))??;
    Ok(())
}

/// Corre el nodo sin interfaz grafica
/// Devuelve un error si no se puede correr el nodo
/// Ok(()) si se corre el nodo correctamente
fn run_without_ui(args: &[String]) -> Result<(), NodeCustomErrors> {
    run_node(args, None, None)
}

/// Corre el nodo con o sin interfaz grafica segun los argumentos
/// Devuelve un error si no se puede correr el nodo
/// Ok(()) si se corre el nodo correctamente
fn run_node(
    args: &[String],
    ui_sender: Option<glib::Sender<UIEvent>>,
    node_rx: Option<Receiver<WalletEvent>>,
) -> Result<(), NodeCustomErrors> {
    wait_for_start_button(&node_rx);
    send_event_to_ui(&ui_sender, UIEvent::StartHandshake);
    let config = Config::from(args)?;
    let (log_sender, log_sender_handles) = set_up_loggers(&config)?;
    let node_ips = get_active_nodes_from_dns_seed(&config, &log_sender)?;
    let nodes = handshake_with_nodes(&config, &log_sender, node_ips)?;
    let blockchain = initial_block_download(&config, &log_sender, &ui_sender, nodes.clone())?;
    let mut node = Node::new(&log_sender, &ui_sender, nodes, blockchain.clone())?;
    send_event_to_ui(
        &ui_sender,
        UIEvent::InitializeUITabs((blockchain.headers, blockchain.blocks)),
    );
    let mut wallet = Wallet::new(node.clone())?;
    let server = NodeServer::new(&config, &log_sender, &ui_sender, &mut node)?;
    handle_ui_events(&ui_sender, node_rx, &mut wallet);
    shut_down(node, server, log_sender, log_sender_handles)?;
    Ok(())
}

/// Espera a que se presione el boton de start en la interfaz grafica. La UI le envia un evento al nodo
/// indicando que se presiono el boton. En caso de no haber interfaz grafica no hace nada
fn wait_for_start_button(rx: &Option<Receiver<WalletEvent>>) {
    if let Some(rx) = rx {
        for event in rx {
            if let WalletEvent::Start = event {
                break;
            }
        }
    }
}

/// Cierra los threads del nodo y del server, cierra los loggers y devuelve un error si no se pueden cerrar
fn shut_down(
    node: Node,
    server: NodeServer,
    log_sender: LogSender,
    log_sender_handles: LogSenderHandles,
) -> Result<(), NodeCustomErrors> {
    node.shutdown_node()?;
    server.shutdown_server()?;
    shutdown_loggers(log_sender, log_sender_handles)?;
    Ok(())
}

/// Recibe un sender que envia eventos a la UI o None, un receiver que recibe eventos de la UI o none y una wallet
/// Si el Receiver es Some se encarga de manejar los eventos de la UI, si es None se encarga de mostar la interfaz de terminal
/// para que el usuario interactue con la wallet
fn handle_ui_events(
    ui_sender: &Option<glib::Sender<UIEvent>>,
    node_rx: Option<Receiver<WalletEvent>>,
    wallet: &mut Wallet,
) {
    if let Some(rx) = node_rx {
        handle_ui_request(ui_sender, rx, wallet)
    } else {
        terminal_ui(ui_sender, wallet)
    }
}
