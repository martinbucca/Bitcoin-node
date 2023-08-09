use crate::{gtk::ui_events::UIEvent, wallet};
use ::gtk::glib;
use wallet::Wallet;

/// Muestra las opciones para interactuar con el programa desde la terminal, espera algun comando
/// y lo handlea o muestra un mensaje de error
pub fn terminal_ui(ui_sender: &Option<glib::Sender<UIEvent>>, wallet: &mut Wallet) {
    show_options();
    loop {
        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(_) => {
                println!("\n");
                let command = input.trim();
                if let Ok(num) = command.parse::<u32>() {
                    match num {
                        0 => {
                            println!("Cerrando nodo...\n");
                            break;
                        }
                        1 => {
                            handle_add_account_request(ui_sender, wallet);
                        }
                        2 => {
                            handle_balance_request(wallet);
                        }
                        3 => {
                            handle_transaccion_request(ui_sender, wallet);
                        }
                        4 => {
                            handle_poi_request(wallet);
                        }
                        _ => {
                            println!("Número no reconocido. Inténtalo de nuevo! \n");
                        }
                    }
                    show_options();
                } else {
                    println!("Entrada inválida. Inténtalo de nuevo! \n");
                }
            }
            Err(error) => {
                println!("Error al leer la entrada: {}", error);
            }
        }
    }
}

// Muestra por terminal los posibles comandos a ingresar para interactuar con la wallet
fn show_options() {
    println!("\n");
    println!("INGRESE ALGUNO DE LOS SIGUIENTES COMANDOS\n");
    println!("0: Terminar el programa");
    println!("1: Añadir una cuenta a la wallet");
    println!("2: Mostrar balance de las cuentas");
    println!("3: Hacer transaccion desde una cuenta");
    println!("4: Prueba de inclusion de una transaccion en un bloque");
    println!("-----------------------------------------------------------\n");
}

/// Le pide al usuario que ingrese por terminal los datos necesarios para hacer una transaccion
/// e intenta hacer una transaccion. En caso de error imprime por la terminal el error
fn handle_transaccion_request(ui_sender: &Option<glib::Sender<UIEvent>>, wallet: &mut Wallet) {
    if wallet.show_indexes_of_accounts().is_err() {
        return;
    }
    println!("INGRESE LOS SIGUIENTES DATOS PARA REALIZAR UNA TRANSACCION \n");
    let account_index: usize = read_input("Índice de la cuenta: ").unwrap_or_else(|err| {
        println!("Error al leer la entrada: {}", err);
        0
    });
    wallet
        .change_account(ui_sender, account_index)
        .unwrap_or_else(|err| println!("Error al cambiar de cuenta: {}", err));
    let address_receiver: String = read_input("Dirección del receptor: ").unwrap_or_else(|err| {
        println!("Error al leer la entrada: {}", err);
        String::new()
    });
    let amount: i64 = read_input("Cantidad(Satoshis): ").unwrap_or_else(|err| {
        println!("Error al leer la entrada: {}", err);
        0
    });
    let fee: i64 = read_input("Tarifa(Satoshis): ").unwrap_or_else(|err| {
        println!("Error al leer la entrada: {}", err);
        0
    });
    println!("Realizando y broadcasteando transaccion...");
    if let Err(error) = wallet.make_transaction(ui_sender, &address_receiver, amount, fee) {
        println!("Error al realizar la transacción: {}", error);
    } else {
        println!("TRANSACCION REALIZADA CORRECTAMENTE!");
    }
}

/// Recibe lo que se quiere pedir por terminal y espera a que se ingrese algo para poder parsearlo
fn read_input<T: std::str::FromStr>(prompt: &str) -> Result<T, std::io::Error>
where
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    println!("{}", prompt);
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let value: T = input.trim().parse().map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Error parsing input: {}", err),
        )
    })?;
    Ok(value)
}

/// Le pide al usuario que ingrese por terminal los datos de la cuenta y la añade a la wallet. En caso de que los
/// datos ingresados sean incorrectos, lo muestra por pantalla.
fn handle_add_account_request(ui_sender: &Option<glib::Sender<UIEvent>>, wallet: &mut Wallet) {
    println!("Ingrese PRIVATE KEY en formato WIF: ");
    let mut private_key_input = String::new();
    match std::io::stdin().read_line(&mut private_key_input) {
        Ok(_) => {
            let wif_private_key = private_key_input.trim();
            println!("Ingrese la ADDRESS COMPRIMIDA de la cuenta: ");
            let mut address_input = String::new();
            match std::io::stdin().read_line(&mut address_input) {
                Ok(_) => {
                    let address = address_input.trim();
                    println!("Agregando la cuenta -- {} -- a la wallet...\n", address);
                    if let Err(err) = wallet.add_account(
                        ui_sender,
                        wif_private_key.to_string(),
                        address.to_string(),
                    ) {
                        println!("ERROR: {err}\n");
                        println!("Ocurrio un error al intentar añadir una nueva cuenta, intente de nuevo! \n");
                    } else {
                        println!(
                            "CUENTA -- {} -- AÑADIDA CORRECTAMENTE A LA WALLET!\n",
                            address
                        );
                    }
                }
                Err(error) => {
                    println!("Error al leer la entrada: {}", error);
                }
            }
        }
        Err(error) => {
            println!("Error al leer la entrada: {}", error);
        }
    }
}

/// Muestra el balance de todas las cuentas de la wallet por pantalla
fn handle_balance_request(wallet: &mut Wallet) {
    println!("Calculando el balance de las cuentas...\n");
    match wallet.show_accounts_balance() {
        Ok(_) => {}
        Err(e) => println!("Error al leer el balance: {}", e),
    }
}

/// Le pide al usuario que ingrese por terminal los hash de bloque y transaccion para realizar la prueba de inclusión. En caso de que los
/// datos ingresados sean incorrectos, lo muestra por pantalla
fn handle_poi_request(wallet: &mut Wallet) {
    println!("Ingrese el hash del bloque: ");
    let mut block_hash_input = String::new();
    match std::io::stdin().read_line(&mut block_hash_input) {
        Ok(_) => {
            let block_hash = block_hash_input.trim();
            println!("Ingrese el hash de la transacción: ");
            let mut txid_input = String::new();
            match std::io::stdin().read_line(&mut txid_input) {
                Ok(_) => {
                    let txid = txid_input.trim();
                    println!("Realizando la proof of inclusion ...\n");

                    let poi = match wallet
                        .tx_proof_of_inclusion(block_hash.to_string(), txid.to_string())
                    {
                        Err(err) => {
                            println!("ERROR: {err}\n");
                            println!("Ocurrio un error al realizar la proof of inclusion, intente de nuevo! \n");
                            return;
                        }
                        Ok(poi) => poi,
                    };
                    match poi {
                        true => println!("La transacción se encuentra en el bloque."),
                        false => println!("La transacción no se encuentra en el bloque."),
                    }
                }
                Err(error) => {
                    println!("Error al leer la entrada: {}", error);
                }
            }
        }
        Err(error) => {
            println!("Error al leer la entrada: {}", error);
        }
    }
}
