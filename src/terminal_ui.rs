use crate::{gtk::ui_events::UIEvent, wallet};
use ::gtk::glib;
use wallet::Wallet;

/// Shows the options to interact with the program from the terminal, waits for some command
/// and handle it or shows an error message
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
                            println!("Closing node...\n");
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
                            println!("Not valid number. Try again! \n");
                        }
                    }
                    show_options();
                } else {
                    println!("Invalid input. Try again! \n");
                }
            }
            Err(error) => {
                println!("Error trying to read input: {}", error);
            }
        }
    }
}

/// Shows the possible commands to interact with the wallet
fn show_options() {
    println!("\n");
    println!("Enter the number of the command you want to execute: \n");
    println!("0: End the program");
    println!("1: Add an account to the wallet");
    println!("2: Show the balance of the accounts");
    println!("3: Make a transaction from an account");
    println!("4: Proof of inclusion of a transaction in a block");
    println!("-----------------------------------------------------------\n");
}

/// Asks the user to enter the data necessary to make a transaction by terminal
/// and tries to make a transaction. In case of error it prints the error by terminal
fn handle_transaccion_request(ui_sender: &Option<glib::Sender<UIEvent>>, wallet: &mut Wallet) {
    if wallet.show_indexes_of_accounts().is_err() {
        return;
    }
    println!("ENTER THE FOLLOWING DATA TO MAKE A TRANSACTION \n");
    let account_index: usize = read_input("Account index: ").unwrap_or_else(|err| {
        println!("Error trying to read the input: {}", err);
        0
    });
    wallet
        .change_account(ui_sender, account_index)
        .unwrap_or_else(|err| println!("Error trying to change account: {}", err));
    let address_receiver: String = read_input("Reciever address: ").unwrap_or_else(|err| {
        println!("Error trying to read the input: {}", err);
        String::new()
    });
    let amount: i64 = read_input("Amount(Satoshis): ").unwrap_or_else(|err| {
        println!("Error trying to read the input: {}", err);
        0
    });
    let fee: i64 = read_input("Fee(Satoshis): ").unwrap_or_else(|err| {
        println!("Error trying to read the input: {}", err);
        0
    });
    println!("Broadcasting transaction...");
    if let Err(error) = wallet.make_transaction(ui_sender, &address_receiver, amount, fee) {
        println!("Error trying to make the transaction: {}", error);
    } else {
        println!("TRANSACTION MADE SUCCESSFULLY!");
    }
}

/// Receives what you want to ask for by terminal and waits for something to be entered to be able to parse it
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

/// Asks the user to enter the account data by terminal and adds it to the wallet. In case of error when adding the account
/// shows it on the screen
fn handle_add_account_request(ui_sender: &Option<glib::Sender<UIEvent>>, wallet: &mut Wallet) {
    println!("Enter the  PRIVATE KEY in WIF format: ");
    let mut private_key_input = String::new();
    match std::io::stdin().read_line(&mut private_key_input) {
        Ok(_) => {
            let wif_private_key = private_key_input.trim();
            println!("Enter the ADDRESS (compressed) of the account: ");
            let mut address_input = String::new();
            match std::io::stdin().read_line(&mut address_input) {
                Ok(_) => {
                    let address = address_input.trim();
                    println!("Adding account -- {} -- to wallet...\n", address);
                    if let Err(err) = wallet.add_account(
                        ui_sender,
                        wif_private_key.to_string(),
                        address.to_string(),
                    ) {
                        println!("ERROR: {err}\n");
                        println!("An error occurred while trying to add a new account, try again! \n");
                    } else {
                        println!("ACCOUNT -- {} -- ADDED CORRECTLY TO THE WALLET!\n", address);
                    }
                }
                Err(error) => {
                    println!("Error trying to read the input: {}", error);
                }
            }
        }
        Err(error) => {
            println!("Error trying to read the input: {}", error);
        }
    }
}

/// Shows the balance of all accounts in the wallet on the screen
fn handle_balance_request(wallet: &mut Wallet) {
    println!("Calculating balance of all the accounts...\n");
    match wallet.show_accounts_balance() {
        Ok(_) => {}
        Err(e) => println!("Error trying to get the balance: {}", e),
    }
}

/// Asks the user to enter the block and transaction hash by terminal to perform the proof of inclusion. In case the
/// data entered is incorrect, it shows it on the screen
fn handle_poi_request(wallet: &mut Wallet) {
    println!("Enter the hash of the block: ");
    let mut block_hash_input = String::new();
    match std::io::stdin().read_line(&mut block_hash_input) {
        Ok(_) => {
            let block_hash = block_hash_input.trim();
            println!("Enter the hash of the transaction: ");
            let mut txid_input = String::new();
            match std::io::stdin().read_line(&mut txid_input) {
                Ok(_) => {
                    let txid = txid_input.trim();
                    println!("Calculating proof of inclusion...\n");

                    let poi = match wallet
                        .tx_proof_of_inclusion(block_hash.to_string(), txid.to_string())
                    {
                        Err(err) => {
                            println!("ERROR: {err}\n");
                            println!("An error occurred while trying to calculate the proof of inclusion, try again! \n");
                            return;
                        }
                        Ok(poi) => poi,
                    };
                    match poi {
                        true => println!("The transaction is in the block."),
                        false => println!("The transaction is not in the block."),
                    }
                }
                Err(error) => {
                    println!("Error trying to read the input: {}", error);
                }
            }
        }
        Err(error) => {
            println!("Error trying to read the input: {}", error);
        }
    }
}
