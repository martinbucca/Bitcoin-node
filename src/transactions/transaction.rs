use std::{
    collections::HashMap,
    error::Error,
    io,
    sync::{Arc, RwLock},
};

use bitcoin_hashes::{sha256, sha256d, Hash};
use gtk::glib;

use crate::{
    account::Account, compact_size_uint::CompactSizeUint, custom_errors::NodeCustomErrors,
    gtk::ui_events::UIEvent, logwriter::log_writer::LogSender, utxo_tuple::UtxoTuple,
};

use super::{
    outpoint::Outpoint,
    script::{
        p2pkh_script::{self, generate_pubkey_script},
        sig_script::SigScript,
    },
    tx_in::TxIn,
    tx_out::TxOut,
};

const SIG_HASH_ALL: u32 = 0x00000001;
const TRANSACTION_VERSION: i32 = 0x00000002;

/// Representa una transacción del protocolo bitcoin
#[derive(Debug, PartialEq, Clone)]
pub struct Transaction {
    pub version: i32,
    pub txin_count: CompactSizeUint,
    pub tx_in: Vec<TxIn>,
    pub txout_count: CompactSizeUint,
    pub tx_out: Vec<TxOut>,
    pub lock_time: u32,
}

impl Transaction {
    /// Crea la transacción con los parámetros recibidos.
    pub fn new(
        version: i32,
        txin_count: CompactSizeUint,
        tx_in: Vec<TxIn>,
        txout_count: CompactSizeUint,
        tx_out: Vec<TxOut>,
        lock_time: u32,
    ) -> Self {
        Transaction {
            version,
            txin_count,
            tx_in,
            txout_count,
            tx_out,
            lock_time,
        }
    }

    /// Deserializa la transacción a partir de una cadena de bytes.
    /// Devuelve la transacción o un error en caso de que la cadena no cumpla con el formato
    pub fn unmarshalling(bytes: &Vec<u8>, offset: &mut usize) -> Result<Transaction, &'static str> {
        // en teoria se lee el coinbase transaccion primero
        if bytes.len() < 10 {
            return Err(
                "Los bytes recibidos no corresponden a un Transaction, el largo es menor a 10 bytes",
            );
        }
        let mut version_bytes: [u8; 4] = [0; 4];
        version_bytes.copy_from_slice(&bytes[*offset..(*offset + 4)]);
        *offset += 4;
        let version = i32::from_le_bytes(version_bytes);
        let txin_count: CompactSizeUint = CompactSizeUint::unmarshalling(bytes, &mut *offset)?;
        let amount_txin: u64 = txin_count.decoded_value();
        let tx_in: Vec<TxIn> = TxIn::unmarshalling_txins(bytes, amount_txin, &mut *offset)?; // aca se actualizaria el *offset tambien
        if tx_in[0].is_coinbase() && txin_count.decoded_value() != 1 {
            return Err("una coinbase transaction no puede tener mas de un input");
        }
        let txout_count: CompactSizeUint = CompactSizeUint::unmarshalling(bytes, &mut *offset)?;
        let amount_txout: u64 = txout_count.decoded_value();
        let tx_out: Vec<TxOut> = TxOut::unmarshalling_txouts(bytes, amount_txout, &mut *offset)?; // aca se actualizaria el *offset tambien
        let mut lock_time_bytes: [u8; 4] = [0; 4];
        lock_time_bytes.copy_from_slice(&bytes[*offset..(*offset + 4)]);
        *offset += 4;
        let lock_time = u32::from_le_bytes(lock_time_bytes);
        Ok(Transaction {
            version,
            txin_count,
            tx_in,
            txout_count,
            tx_out,
            lock_time,
        })
    }

    /// Serializa la transacción.
    /// Guarda los bytes en la referencia del vector recibido.
    pub fn marshalling(&self, bytes: &mut Vec<u8>) {
        let version_bytes: [u8; 4] = self.version.to_le_bytes();
        bytes.extend_from_slice(&version_bytes);
        bytes.extend_from_slice(&self.txin_count.marshalling());
        for tx_in in &self.tx_in {
            tx_in.marshalling(bytes);
        }
        bytes.extend_from_slice(&self.txout_count.marshalling());
        for tx_out in &self.tx_out {
            tx_out.marshalling(bytes);
        }
        let locktime_bytes: [u8; 4] = self.lock_time.to_le_bytes();
        bytes.extend_from_slice(&locktime_bytes);
    }
    ///Devuelve el hash de la transaccion
    pub fn hash(&self) -> [u8; 32] {
        self.hash_message(false)
    }
    /// Realiza el hash de la transaccion.
    /// Si recibe true pushea dentro del vector los bytes correspondientes al SIGHASH_ALL.
    /// Caso contrario realiza el hash normalmente
    fn hash_message(&self, is_message: bool) -> [u8; 32] {
        let mut raw_transaction_bytes: Vec<u8> = Vec::new();
        self.marshalling(&mut raw_transaction_bytes);
        if is_message {
            let bytes = SIG_HASH_ALL.to_le_bytes();
            raw_transaction_bytes.extend_from_slice(&bytes);
        }
        if is_message {
            let hash_transaction = sha256::Hash::hash(&raw_transaction_bytes);
            return *hash_transaction.as_byte_array();
        }
        let hash_transaction = sha256d::Hash::hash(&raw_transaction_bytes);
        *hash_transaction.as_byte_array()
    }

    /// Recibe una referencia a un vector de bytes y la cantidad de transacciones a deserializar.
    /// Devuelve un vector con las transacciones o error.
    /// Actualiza el offset
    pub fn unmarshalling_transactions(
        bytes: &Vec<u8>,
        amount_transactions: u64,
        offset: &mut usize,
    ) -> Result<Vec<Transaction>, &'static str> {
        let mut transactions_list: Vec<Transaction> = Vec::new();
        let mut i = 0;
        while i < amount_transactions {
            transactions_list.push(Self::unmarshalling(bytes, offset)?);
            i += 1;
        }
        Ok(transactions_list)
    }

    /// Devuelve true o false dependiendo si la transacción es una coinbase
    pub fn is_coinbase_transaction(&self) -> bool {
        self.tx_in[0].is_coinbase()
    }

    /// Devuelve una copia de los tx_out de la transaccion
    pub fn get_txout(&self) -> Vec<TxOut> {
        self.tx_out.clone()
    }

    /// Revisa los inputs de la transacción y remueve las utxos que fueron gastadas
    pub fn remove_utxos(
        &self,
        utxo_set: Arc<RwLock<HashMap<[u8; 32], UtxoTuple>>>,
    ) -> Result<(), Box<dyn Error>> {
        // Si la tx gasta un output existente en nuestro utxo_set, lo removemos
        for txin in &self.tx_in {
            let txid = &txin.get_previous_output_hash();
            let output_index = txin.get_previous_output_index();
            if utxo_set
                .read()
                .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                .contains_key(txid)
            {
                if let Some(utxo) = utxo_set
                    .write()
                    .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                    .get_mut(txid)
                {
                    utxo.remove_utxo(output_index);
                }
            }
        }
        Ok(())
    }

    /// Genera el UtxoTuple y lo guarda en el utxo_set
    pub fn load_utxos(
        &self,
        utxo_set: Arc<RwLock<HashMap<[u8; 32], UtxoTuple>>>,
    ) -> Result<(), Box<dyn Error>> {
        let hash = self.hash();
        let mut utxos_and_index = Vec::new();
        for (position, utxo) in self.tx_out.iter().enumerate() {
            let utxo_and_index = (utxo.clone(), position);
            utxos_and_index.push(utxo_and_index);
        }
        let utxo_tuple = UtxoTuple::new(hash, utxos_and_index);
        utxo_set
            .write()
            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            .insert(hash, utxo_tuple);
        Ok(())
    }

    /// Devuelve un string que representa el hash de la transaccion en hexadecimal y en el formato
    /// que se usa en la pagina https://blockstream.info/testnet/ para mostrar transacciones
    pub fn hex_hash(&self) -> String {
        let hash_as_bytes = self.hash();
        let inverted_hash: [u8; 32] = {
            let mut inverted = [0; 32];
            for (i, byte) in hash_as_bytes.iter().enumerate() {
                inverted[31 - i] = *byte;
            }
            inverted
        };
        let hex_hash = inverted_hash
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect();
        hex_hash
    }

    /// Recibe un puntero a un puntero con las cuentas de la wallet y se fija si alguna tx_out tiene una address
    /// igual que alguna de la wallet. Devuelve Ok(()) en caso de no ocurrir ningun error o Error especifico en caso contrario
    pub fn check_if_tx_involves_user_account(
        &self,
        log_sender: &LogSender,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        accounts: Arc<RwLock<Arc<RwLock<Vec<Account>>>>>,
    ) -> Result<(), NodeCustomErrors> {
        for tx_out in self.tx_out.clone() {
            tx_out.involves_user_account(log_sender, ui_sender, accounts.clone(), self.clone())?;
        }
        Ok(())
    }
    /// Esta funcion genera la transaccion sin firmar , los parametros indican la adrress
    /// donde se enviara el monto(value), la recompensa por agregar la nueva transaccion
    /// al bloque(fee) y la direccion para retornar el cambio en caso de que se genere(change_address)
    pub fn generate_unsigned_transaction(
        address_receiver: &str,
        change_adress: &str,
        value: i64,
        fee: i64,
        utxos_to_spend: &Vec<UtxoTuple>,
    ) -> Result<Transaction, Box<dyn Error>> {
        let mut tx_ins: Vec<TxIn> = Vec::new();
        let mut input_balance: i64 = 0;
        // en esta parte se generan los tx_in con la referencia de los utxos
        // de donde obtenemos los satoshis para ser gastados ,¡ojo! pueden ser mas de uno.
        for utxo in utxos_to_spend {
            let tx_id: [u8; 32] = utxo.hash();
            input_balance += utxo.balance();
            let indexes: Vec<usize> = utxo.get_indexes_from_utxos();
            for index in indexes {
                let previous_output: Outpoint = Outpoint::new(tx_id, index as u32);
                let tx_in: TxIn = TxIn::incomplete_txin(previous_output);
                tx_ins.push(tx_in);
            }
        }
        // esta variable contiene el monto correspondiente al sobrante de la tx
        let change_amount: i64 = input_balance - (value + fee);
        // esta variable indica la cantidad de txIn creados en los pasos anteriores
        let txin_count: CompactSizeUint = CompactSizeUint::new(tx_ins.len() as u128);
        // este vector contiene los outputs de nuestra transaccion
        let mut tx_outs: Vec<TxOut> = Vec::new();
        // creacion del pubkey_script donde transferimos los satoshis
        let target_pk_script: Vec<u8> = generate_pubkey_script(address_receiver)?;
        let target_pk_script_bytes: CompactSizeUint =
            CompactSizeUint::new(target_pk_script.len() as u128);
        // creacion del txOut(utxo) referenciado al address que nos enviaron
        let utxo_to_send: TxOut = TxOut::new(value, target_pk_script_bytes, target_pk_script);
        tx_outs.push(utxo_to_send);
        // creacion del pubkey_script donde enviaremos el cambio de nuestra tx
        let change_pk_script: Vec<u8> = generate_pubkey_script(change_adress)?;
        let change_pk_script_bytes: CompactSizeUint =
            CompactSizeUint::new(change_pk_script.len() as u128);
        let change_utxo: TxOut =
            TxOut::new(change_amount, change_pk_script_bytes, change_pk_script);
        tx_outs.push(change_utxo);
        let txout_count = CompactSizeUint::new(tx_outs.len() as u128);
        // lock_time = 0 => Not locked
        let lock_time: u32 = 0;
        let incomplete_transaction = Transaction::new(
            TRANSACTION_VERSION,
            txin_count,
            tx_ins,
            txout_count,
            tx_outs,
            lock_time,
        );
        Ok(incomplete_transaction)
    }

    /// Firma la transacción.
    /// Recibe la lista de utxos a gastar y agrega el signature_script a cada TxIn.
    pub fn sign(
        &mut self,
        account: &Account,
        utxos_to_spend: &Vec<UtxoTuple>,
    ) -> Result<(), Box<dyn Error>> {
        let mut signatures = Vec::new();
        for index in 0..self.tx_in.len() {
            // agregar el signature a cada input
            let z = self.generate_message_to_sign(index, utxos_to_spend);
            signatures.push(SigScript::generate_sig_script(z, account)?);
        }
        for (index, signature) in signatures.into_iter().enumerate() {
            self.tx_in[index].add(signature);
        }
        Ok(())
    }

    /// Genera la txin con el previous pubkey del tx_in recibido.
    /// Devuelve el hash
    fn generate_message_to_sign(
        &self,
        tx_in_index: usize,
        utxos_to_spend: &Vec<UtxoTuple>,
    ) -> [u8; 32] {
        let mut tx_copy = self.clone();
        let mut script = Vec::new();
        let input_to_sign = &tx_copy.tx_in[tx_in_index];
        for utxos in utxos_to_spend {
            let pubkey = utxos.find(
                input_to_sign.get_previous_output_hash(),
                input_to_sign.get_previous_output_index(),
            );
            script = match pubkey {
                Some(value) => value.to_vec(),
                None => continue,
            };
        }
        tx_copy.tx_in[tx_in_index].set_signature_script(script);
        tx_copy.hash_message(true)
    }

    /// Valida la transacción.
    /// Ejecuta el script y devuelve error en caso de que no pase la validación.
    pub fn validate(&self, utxos_to_spend: &Vec<UtxoTuple>) -> Result<(), Box<dyn Error>> {
        let mut p2pkh_scripts = Vec::new();
        for utxo in utxos_to_spend {
            for (txout, _) in &utxo.utxo_set {
                p2pkh_scripts.push(txout.get_pub_key_script())
            }
        }

        for (index, txin) in self.tx_in.iter().enumerate() {
            //txin.
            if !p2pkh_script::validate(p2pkh_scripts[index], txin.signature_script.get_bytes())? {
                return Err(Box::new(std::io::Error::new(
                    io::ErrorKind::Other,
                    "El p2pkh_script no pasó la validación.",
                )));
            }
        }
        Ok(())
    }

    /// Devuelve el monto de la transacción.
    pub fn amount(&self) -> i64 {
        let mut amount = 0;
        for txout in &self.tx_out {
            amount += txout.value();
        }
        amount
    }
    /// Devuelve la altura del bloque en el que se encuentra la transacción.
    /// Válido sólo para las coinbase transactions.
    pub fn get_height(&self) -> u32 {
        self.tx_in[0].get_height()
    }

    /// Devuelve el monto enviado a direcciones distintas de la recibida por parámetro
    pub fn amount_spent_by_account(&self, address: &String) -> Result<i64, Box<dyn Error>> {
        let mut amount = 0;
        for txout in &self.tx_out {
            if !txout.is_sent_to_account(address)? {
                amount += txout.value();
            }
        }
        Ok(amount)
    }
}

#[cfg(test)]

mod test {
    use super::Transaction;
    use crate::{
        compact_size_uint::CompactSizeUint,
        transactions::script::sig_script::SigScript,
        transactions::{outpoint::Outpoint, tx_in::TxIn, tx_out::TxOut},
    };
    use bitcoin_hashes::{sha256d, Hash};

    /// Funcion auxiliar que crea los txin
    fn crear_txins(cantidad: u128) -> Vec<TxIn> {
        let mut tx_in: Vec<TxIn> = Vec::new();
        for _i in 0..cantidad {
            let tx_id: [u8; 32] = [1; 32];
            let index_outpoint: u32 = 0x30000000;
            let outpoint: Outpoint = Outpoint::new(tx_id, index_outpoint);
            let compact_txin: CompactSizeUint = CompactSizeUint::new(1);
            let bytes: Vec<u8> = vec![1];
            let signature_script = SigScript::new(bytes);
            let sequence: u32 = 0xffffffff;
            tx_in.push(TxIn::new(
                outpoint,
                compact_txin,
                None,
                signature_script,
                sequence,
            ));
        }
        tx_in
    }

    /// Funcion auxiliar que crea los txout
    fn crear_txouts(cantidad: u128) -> Vec<TxOut> {
        let mut tx_out: Vec<TxOut> = Vec::new();
        for _i in 0..cantidad {
            let value: i64 = 43;
            let pk_script_bytes: CompactSizeUint = CompactSizeUint::new(0);
            let pk_script: Vec<u8> = Vec::new();
            tx_out.push(TxOut::new(value, pk_script_bytes, pk_script));
        }
        tx_out
    }

    /// Funcion auxiliar que crea la cadena de bytes para testear la deserializacion
    fn generar_flujo_de_datos(
        version: i32,
        tx_in_count: u128,
        tx_out_count: u128,
        lock_time: u32,
    ) -> Vec<u8> {
        //contenedor de bytes
        let mut bytes: Vec<u8> = Vec::new();
        // version settings
        let version: i32 = version;
        // tx_in_count settings
        let txin_count = CompactSizeUint::new(tx_in_count);
        // tx_in settings
        let tx_in: Vec<TxIn> = crear_txins(tx_in_count);
        // tx_out_count settings
        let txout_count = CompactSizeUint::new(tx_out_count);
        // tx_out settings
        let tx_out: Vec<TxOut> = crear_txouts(tx_out_count);
        //lock_time settings
        let lock_time: u32 = lock_time;
        let transaction: Transaction =
            Transaction::new(version, txin_count, tx_in, txout_count, tx_out, lock_time);
        transaction.marshalling(&mut bytes);
        bytes
    }

    #[test]
    fn test_la_transaccion_se_hashea_correctamente() {
        let previous_output: Outpoint = Outpoint::new([1; 32], 0x11111111);
        let script_bytes: CompactSizeUint = CompactSizeUint::new(0);
        let mut tx_in: Vec<TxIn> = Vec::new();
        tx_in.push(TxIn::new(
            previous_output,
            script_bytes,
            None,
            SigScript::new(Vec::new()),
            0x11111111,
        ));
        let pk_script_bytes: CompactSizeUint = CompactSizeUint::new(0);
        let mut tx_out: Vec<TxOut> = Vec::new();
        tx_out.push(TxOut::new(0x1111111111111111, pk_script_bytes, Vec::new()));
        let txin_count: CompactSizeUint = CompactSizeUint::new(1);
        let txout_count: CompactSizeUint = CompactSizeUint::new(1);
        let transaction: Transaction = Transaction::new(
            0x11111111,
            txin_count,
            tx_in,
            txout_count,
            tx_out,
            0x11111111,
        );
        let mut vector = Vec::new();
        transaction.marshalling(&mut vector);
        let hash_transaction = sha256d::Hash::hash(&vector);
        assert_eq!(transaction.hash(), *hash_transaction.as_byte_array());
    }

    #[test]
    fn test_unmarshalling_transaction_invalida() {
        let bytes: Vec<u8> = vec![0; 5];

        let mut offset: usize = 0;
        let transaction = Transaction::unmarshalling(&bytes, &mut offset);
        assert!(transaction.is_err());
    }

    #[test]
    fn test_unmarshalling_transaction_con_coinbase_y_mas_inputs_devuelve_error() {
        //contenedor de bytes
        let mut bytes: Vec<u8> = Vec::new();
        // version settings
        let version: i32 = 23;
        let version_bytes = version.to_le_bytes();
        bytes.extend_from_slice(&version_bytes[0..4]);
        // tx_in_count settings
        let txin_count = CompactSizeUint::new(2);
        bytes.extend_from_slice(&txin_count.marshalling()[0..1]);
        // tx_in settings
        let tx_id: [u8; 32] = [0; 32];
        let index_outpoint: u32 = 0xffffffff;
        let outpoint: Outpoint = Outpoint::new(tx_id, index_outpoint);
        let compact_txin: CompactSizeUint = CompactSizeUint::new(5);
        let height = Some(vec![1, 1, 1, 1]);
        let bytes_to_sig: Vec<u8> = vec![1];
        let signature_script = SigScript::new(bytes_to_sig);
        let sequence: u32 = 0xffffffff;
        let mut tx_in: Vec<TxIn> = Vec::new();
        tx_in.push(TxIn::new(
            outpoint,
            compact_txin,
            height,
            signature_script,
            sequence,
        ));
        tx_in[0 as usize].marshalling(&mut bytes);
        let cantidad_txin: u128 = txin_count.decoded_value() as u128;
        let tx_input: Vec<TxIn> = crear_txins(cantidad_txin);
        tx_input[0 as usize].marshalling(&mut bytes);
        // tx_out_count settings
        let txout_count = CompactSizeUint::new(1);
        bytes.extend_from_slice(txout_count.value());
        // tx_out settings
        let cantidad_txout: u128 = txout_count.decoded_value() as u128;
        let tx_out: Vec<TxOut> = crear_txouts(cantidad_txout);
        tx_out[0 as usize].marshalling(&mut bytes);
        //lock_time settings
        let lock_time: [u8; 4] = [0; 4];
        bytes.extend_from_slice(&lock_time);

        let mut offset: usize = 0;
        let transaction: Result<Transaction, &'static str> =
            Transaction::unmarshalling(&bytes, &mut offset);
        assert!(transaction.is_err());
    }

    #[test]
    fn test_unmarshalling_transaction_devuelve_version_esperado() -> Result<(), &'static str> {
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generar_flujo_de_datos(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(transaction.version, version);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_devuelve_txin_count_esperado() -> Result<(), &'static str> {
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generar_flujo_de_datos(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        let tx_count_expected: CompactSizeUint = CompactSizeUint::new(tx_in_count);
        assert_eq!(transaction.txin_count, tx_count_expected);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_devuelve_txin_esperado() -> Result<(), &'static str> {
        let tx_in_count: u128 = 1;
        let tx_out_count: u128 = 1;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generar_flujo_de_datos(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        let tx_in: Vec<TxIn> = crear_txins(tx_in_count);
        assert_eq!(transaction.tx_in, tx_in);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_devuelve_txout_count_esperado() -> Result<(), &'static str> {
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generar_flujo_de_datos(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        let tx_count_expected: CompactSizeUint = CompactSizeUint::new(tx_out_count);
        assert_eq!(transaction.txout_count, tx_count_expected);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_devuelve_txout_esperado() -> Result<(), &'static str> {
        let tx_in_count: u128 = 1;
        let tx_out_count: u128 = 1;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generar_flujo_de_datos(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        let tx_out: Vec<TxOut> = crear_txouts(tx_out_count);
        assert_eq!(transaction.tx_out[0], tx_out[0]);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_devuelve_lock_time_esperado() -> Result<(), &'static str> {
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generar_flujo_de_datos(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(transaction.lock_time, lock_time);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_devuelve_tamanio_txin_esperado() -> Result<(), &'static str> {
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generar_flujo_de_datos(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        assert_eq!(transaction.tx_in.len(), tx_in_count as usize);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_devuelve_vector_txin_esperado() -> Result<(), &'static str> {
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generar_flujo_de_datos(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        let tx_in: Vec<TxIn> = crear_txins(tx_in_count);
        assert_eq!(transaction.tx_in, tx_in);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_transaction_devuelve_vector_txout_esperado() -> Result<(), &'static str> {
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let bytes = generar_flujo_de_datos(version, tx_in_count, tx_out_count, lock_time);
        let mut offset: usize = 0;
        let transaction: Transaction = Transaction::unmarshalling(&bytes, &mut offset)?;
        let tx_out: Vec<TxOut> = crear_txouts(tx_out_count);
        assert_eq!(transaction.tx_out, tx_out);
        Ok(())
    }

    #[test]
    fn test_unmarshalling_de_2_transactions_devuelve_longitud_esperada() -> Result<(), &'static str>
    {
        let tx_in_count: u128 = 1;
        let tx_out_count: u128 = 1;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let mut bytes = generar_flujo_de_datos(version, tx_in_count, tx_out_count, lock_time);
        let bytes2 = generar_flujo_de_datos(version, tx_in_count, tx_out_count, lock_time);
        bytes.extend_from_slice(&bytes2[0..bytes2.len()]);
        let mut offset: usize = 0;
        let transaction: Vec<Transaction> =
            Transaction::unmarshalling_transactions(&bytes, 2, &mut offset)?;
        assert_eq!(transaction.len(), 2);
        Ok(())
    }
}
