use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::sync::Arc;
use std::sync::RwLock;

use crate::address_decoder;
use crate::custom_errors::NodeCustomErrors;
use crate::transactions::transaction::Transaction;
use crate::utxo_tuple::UtxoTuple;
#[derive(Debug, Clone)]
/// Representa una cuenta bitcoin
/// Guarda la address comprimida y la private key (comprimida o no)
/// También guarda las utxos de la cuenta, transacciones pendientes y confirmadas
pub struct Account {
    pub private_key: String,
    pub address: String,
    pub utxo_set: Vec<UtxoTuple>,
    pub pending_transactions: Arc<RwLock<Vec<Transaction>>>,
    pub confirmed_transactions: Arc<RwLock<Vec<Transaction>>>,
}

type TransactionInfo = (String, Transaction, i64);
impl Account {
    /// Recibe la address en formato comprimido
    /// Y la WIF private key, ya sea en formato comprimido o no comprimido
    pub fn new(wif_private_key: String, address: String) -> Result<Account, Box<dyn Error>> {
        let raw_private_key = address_decoder::decode_wif_private_key(wif_private_key.as_str())?;

        address_decoder::validate_address_private_key(&raw_private_key, &address)?;
        Ok(Account {
            private_key: wif_private_key,
            address,
            utxo_set: Vec::new(),
            pending_transactions: Arc::new(RwLock::new(Vec::new())),
            confirmed_transactions: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Devuelve la clave publica comprimida (33 bytes) a partir de la privada
    pub fn get_pubkey_compressed(&self) -> Result<[u8; 33], Box<dyn Error>> {
        address_decoder::get_pubkey_compressed(&self.private_key)
    }
    /// Devuelve la private key decodificada en formato bytes.
    pub fn get_private_key(&self) -> Result<[u8; 32], Box<dyn Error>> {
        address_decoder::decode_wif_private_key(self.private_key.as_str())
    }

    /// Devuelve la dirección de la cuenta
    pub fn get_address(&self) -> &String {
        &self.address
    }
    /// Guarda los utxos en la cuenta
    pub fn load_utxos(&mut self, utxos: Vec<UtxoTuple>) {
        self.utxo_set = utxos;
    }

    /// Compara el monto recibido con el balance de la cuenta.
    /// Devuelve true si el balance es mayor. Caso contrario false
    pub fn has_balance(&self, value: i64) -> bool {
        self.balance() > value
    }

    /// Devuelve el balance de la cuenta
    pub fn balance(&self) -> i64 {
        let mut balance: i64 = 0;
        for utxo in &self.utxo_set {
            balance += utxo.balance();
        }
        balance
    }
    /// Devuelve un vector con las utxos a ser gastadas en una transaccion nueva, según el monto recibido.
    fn get_utxos_for_amount(&mut self, value: i64) -> Vec<UtxoTuple> {
        let mut utxos_to_spend = Vec::new();
        let mut partial_amount: i64 = 0;
        let mut position: usize = 0;
        let lenght: usize = self.utxo_set.len();
        while position < lenght {
            if (partial_amount + self.utxo_set[position].balance()) < value {
                partial_amount += self.utxo_set[position].balance();
                utxos_to_spend.push(self.utxo_set[position].clone());
                // No corresponde removerlas mientras la tx no está confirmada
            } else {
                utxos_to_spend
                    .push(self.utxo_set[position].utxos_to_spend(value, &mut partial_amount));
                break;
            }
            position += 1;
        }
        utxos_to_spend
    }

    /// Agrega la transacción a la lista de transacciones pendientes.
    fn add_transaction(&self, transaction: Transaction) -> Result<(), Box<dyn Error>> {
        let mut aux = self
            .pending_transactions
            .write()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?;
        aux.push(transaction);
        Ok(())
    }
    /// Realiza la transaccion con el monto recibido, devuelve el hash de dicha transaccion
    /// para que el nodo envie dicho hash a lo restantes nodos de la red
    pub fn make_transaction(
        &mut self,
        address_receiver: &str,
        amount: i64,
        fee: i64,
    ) -> Result<Transaction, Box<dyn Error>> {
        address_decoder::validate_address(address_receiver)?;
        if !self.has_balance(amount + fee) {
            return Err(Box::new(std::io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "El balance de la cuenta {} tiene menos de {} satoshis",
                    self.address,
                    amount + fee,
                ),
            )));
        }
        // Sabemos que tenemos monto para realizar la transaccion , ahora debemos obtener las utxos
        // que utilizaremos para gastar
        let utxos_to_spend: Vec<UtxoTuple> = self.get_utxos_for_amount(amount + fee);
        let change_address: &str = self.address.as_str();
        let mut unsigned_transaction = Transaction::generate_unsigned_transaction(
            address_receiver,
            change_address,
            amount,
            fee,
            &utxos_to_spend,
        )?;
        unsigned_transaction.sign(self, &utxos_to_spend)?;
        // el mensaje cifrado creo que no hace falta chequearlo
        unsigned_transaction.validate(&utxos_to_spend)?;

        self.add_transaction(unsigned_transaction.clone())?;
        Ok(unsigned_transaction)
    }

    /// Recibe el utxo_set, lo recorre y setea el utxo_set de la cuenta.
    pub fn set_utxos(
        &mut self,
        utxo_set: Arc<RwLock<HashMap<[u8; 32], UtxoTuple>>>,
    ) -> Result<(), Box<dyn Error>> {
        let mut account_utxo_set: Vec<UtxoTuple> = Vec::new();
        for utxo in utxo_set
            .read()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .values()
        {
            let aux_utxo = utxo.referenced_utxos(&self.address);
            let utxo_to_push = match aux_utxo {
                Some(value) => value,
                None => continue,
            };
            account_utxo_set.push(utxo_to_push);
        }
        self.utxo_set = account_utxo_set;
        Ok(())
    }

    /// Devuelve las transacciones pendientes y las confirmadas de la cuenta
    /// Devuelve una lista de tuplas con el estado, transaccion y monto enviado por la cuenta
    pub fn get_transactions(&self) -> Result<Vec<TransactionInfo>, Box<dyn Error>> {
        let mut transactions: Vec<(String, Transaction, i64)> = Vec::new();
        // itero las pending tx
        for tx in self
            .pending_transactions
            .read()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .iter()
        {
            transactions.push((
                "Pending".to_string(),
                tx.clone(),
                tx.amount_spent_by_account(&self.address)?,
            ));
        }

        for tx in self
            .confirmed_transactions
            .read()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .iter()
        {
            transactions.push((
                "Confirmed".to_string(),
                tx.clone(),
                tx.amount_spent_by_account(&self.address)?,
            ));
        }

        Ok(transactions)
    }
}
/// Convierte la cadena de bytes a hexadecimal y la devuelve
pub fn bytes_to_hex_string(bytes: &[u8]) -> String {
    let hex_chars: Vec<String> = bytes.iter().map(|byte| format!("{:02x}", byte)).collect();

    hex_chars.join("")
}

#[cfg(test)]
mod test {

    use crate::account::Account;
    use std::{
        error::Error,
        io,
        sync::{Arc, RwLock},
    };

    /// Convierte el str recibido en hexadecimal, a bytes
    fn string_to_33_bytes(input: &str) -> Result<[u8; 33], Box<dyn Error>> {
        if input.len() != 66 {
            return Err(Box::new(std::io::Error::new(
                io::ErrorKind::Other,
                "El string recibido es inválido. No tiene el largo correcto",
            )));
        }

        let mut result = [0; 33];
        for i in 0..33 {
            let byte_str = &input[i * 2..i * 2 + 2];
            result[i] = u8::from_str_radix(byte_str, 16)?;
        }

        Ok(result)
    }

    #[test]
    fn test_se_genera_correctamente_la_cuenta_con_wif_comprimida() {
        let address_expected: String = String::from("mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV");
        let private_key: String =
            String::from("cMoBjaYS6EraKLNqrNN8DvN93Nnt6pJNfWkYM8pUufYQB5EVZ7SR");
        let account_result = Account::new(private_key, address_expected);
        assert!(account_result.is_ok());
    }

    #[test]
    fn test_se_genera_correctamente_la_cuenta_con_wif_no_comprimida() {
        let address_expected: String = String::from("mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV");
        let private_key: String =
            String::from("91dkDNCCaMp2f91sVQRGgdZRw1QY4aptaeZ4vxEvuG5PvZ9hftJ");
        let account_result = Account::new(private_key, address_expected);
        assert!(account_result.is_ok());
    }

    #[test]
    fn test_no_se_puede_generar_la_cuenta_con_wif_incorrecta() {
        let address_expected: String = String::from("mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV");
        let private_key: String =
            String::from("K1dkDNCCaMp2f91sVQRGgdZRw1QY4aptaeZ4vxEvuG5PvZ9hftJ");
        let account_result = Account::new(private_key, address_expected);
        assert!(account_result.is_err());
    }

    #[test]
    fn test_usuario_devuelve_clave_publica_comprimida_esperada() -> Result<(), Box<dyn Error>> {
        let address = String::from("mpzx6iZ1WX8hLSeDRKdkLatXXPN1GDWVaF");
        let private_key = String::from("cQojsQ5fSonENC5EnrzzTAWSGX8PB4TBh6GunBxcCdGMJJiLULwZ");
        let user = Account {
            private_key,
            address,
            utxo_set: Vec::new(),
            pending_transactions: Arc::new(RwLock::new(Vec::new())),
            confirmed_transactions: Arc::new(RwLock::new(Vec::new())),
        };
        let expected_pubkey = string_to_33_bytes(
            "0345EC0AA86BAF64ED626EE86B4A76C12A92D5F6DD1C1D6E4658E26666153DAFA6",
        )?;
        assert_eq!(user.get_pubkey_compressed()?, expected_pubkey);
        Ok(())
    }

    #[test]
    fn test_no_se_puede_realizar_transaccion_a_una_address_invalida() -> Result<(), Box<dyn Error>>
    {
        let address_expected: String = String::from("mnEvYsxexfDEkCx2YLEfzhjrwKKcyAhMqV");
        let private_key: String =
            String::from("cMoBjaYS6EraKLNqrNN8DvN93Nnt6pJNfWkYM8pUufYQB5EVZ7SR");
        let mut account = Account::new(private_key, address_expected)?;
        let transaction_result =
            account.make_transaction("mocD12x6BV3qK71FwG98h5VWZ4qVsbaoi8", 1000, 10);
        assert!(transaction_result.is_err());
        Ok(())
    }
}
