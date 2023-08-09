use super::{
    block_header::BlockHeader, merkle_tree::MerkleTree, utils_block::concatenate_and_hash,
};
use crate::{
    account::Account,
    compact_size_uint::CompactSizeUint,
    custom_errors::NodeCustomErrors,
    gtk::ui_events::{send_event_to_ui, UIEvent},
    logwriter::log_writer::{write_in_log, LogSender},
    transactions::transaction::Transaction,
    utxo_tuple::UtxoTuple,
};
use gtk::glib;
use std::{
    collections::HashMap,
    error::Error,
    sync::{Arc, RwLock},
};

/// Representa un bloque del protocolo bitcoin.
#[derive(Debug, Clone)]
pub struct Block {
    pub block_header: BlockHeader,
    pub txn_count: CompactSizeUint,
    pub txn: Vec<Transaction>,
}

impl Block {
    /// Inicializa el Bloque con los campos recibidos.
    pub fn new(
        block_header: BlockHeader,
        txn_count: CompactSizeUint,
        txn: Vec<Transaction>,
    ) -> Block {
        Block {
            block_header,
            txn_count,
            txn,
        }
    }

    /// Recibe una cadena de bytes, la deserializa y devuelve el bloque.
    /// Actualiza el offset según la cantidad de bytes que leyó de la cadena.
    pub fn unmarshalling(bytes: &Vec<u8>, offset: &mut usize) -> Result<Block, &'static str> {
        let block_header: BlockHeader = BlockHeader::unmarshalling(bytes, offset)?;
        let txn_count: CompactSizeUint = CompactSizeUint::unmarshalling(bytes, offset)?;
        let amount_transaction: u64 = txn_count.decoded_value();
        let txn: Vec<Transaction> =
            Transaction::unmarshalling_transactions(bytes, amount_transaction, offset)?;
        Ok(Block {
            block_header,
            txn_count,
            txn,
        })
    }

    /// Convierte el bloque a bytes según el protocolo bitcoin.
    /// Guarda dichos bytes en el vector recibido por parámetro.
    pub fn marshalling(&self, bytes: &mut Vec<u8>) {
        self.block_header.marshalling(bytes);
        bytes.extend_from_slice(&self.txn_count.marshalling());
        for tx in &self.txn {
            tx.marshalling(bytes);
        }
    }

    /// Valida el bloque. Primero realiza la proof of work y
    /// Luego realiza la proof of inclusion sobre su lista de transacciones
    pub fn validate(&self) -> (bool, &'static str) {
        //proof of work
        if !self.block_header.validate() {
            return (false, "El bloque no cumple con la dificultad pedida");
        }
        //proof of inclusion
        let merkle_root_hash: [u8; 32] = self.generate_merkle_root();
        if !self
            .block_header
            .is_same_merkle_root_hash(&merkle_root_hash)
        {
            return (
                false,
                "El merkle root generado es distinto al provisto por el block header",
            );
        }
        let mut weight = Vec::new();
        self.marshalling(&mut weight);
        //se prueba que el bloque no exceda mas de 1 MB
        if weight.len() > 1048576 {
            return (false, "El bloque ocupa mas de un megabyte");
        }
        (true, "El bloque es valido")
    }

    /// Genera la raiz del merkle root a partir de los hashes de las transacciones (tx_id)
    /// Reduce los elementos del vector de tx_id, agrupa de a pares, los hashea y guarda nuevamente
    /// En un vector el cual sera procesado recursivamente hasta obtener el merkle root hash.
    pub fn recursive_generation_merkle_root(vector: Vec<[u8; 32]>) -> [u8; 32] {
        let vec_length: usize = vector.len();
        if vec_length == 1 {
            return vector[0];
        }
        let mut upper_level: Vec<[u8; 32]> = Vec::new();
        let mut amount_hashs: usize = 0;
        let mut current_position: usize = 0;
        for tx in &vector {
            amount_hashs += 1;
            if amount_hashs == 2 {
                upper_level.push(concatenate_and_hash(vector[current_position - 1], *tx));
                amount_hashs = 0;
            }
            current_position += 1;
        }
        // si el largo del vector es impar el ultimo elelmento debe concatenarse consigo
        // mismo y luego aplicarse la funcion de hash
        if (vec_length % 2) != 0 {
            upper_level.push(concatenate_and_hash(
                vector[current_position - 1],
                vector[current_position - 1],
            ));
        }
        Self::recursive_generation_merkle_root(upper_level)
    }

    /// Genera la raiz del merkle root
    pub fn generate_merkle_root(&self) -> [u8; 32] {
        let mut merkle_transactions: Vec<[u8; 32]> = Vec::new();
        for tx in &self.txn {
            merkle_transactions.push(tx.hash());
        }
        Self::recursive_generation_merkle_root(merkle_transactions)
    }
    pub fn is_same_block(&self, block_id: &[u8; 32]) -> bool {
        self.block_header.hash() == *block_id
    }

    /// Actualiza el utxo_set recibido por parámetro.
    /// Procesa las transacciones del bloque. Agrega las nuevas utxos y remueve las gastadas.
    pub fn give_me_utxos(
        &self,
        utxo_set: Arc<RwLock<HashMap<[u8; 32], UtxoTuple>>>,
    ) -> Result<(), Box<dyn Error>> {
        for tx in &self.txn {
            if tx.is_coinbase_transaction() {
                // como se trata de una coinbase al ser la primera tx solo se cargaran
                // las utxos de esta transaccion
                tx.load_utxos(utxo_set.clone())?;
            } else {
                //primero removemos las utxos que usa esta tx
                tx.remove_utxos(utxo_set.clone())?;
                //luego cargamos las utxos de esta tx para que en la siguiente iteracion
                //se remuevan aquellas con son usadas
                tx.load_utxos(utxo_set.clone())?;
            }
        }
        Ok(())
    }
    pub fn merkle_proof_of_inclusion(
        &self,
        tx_id_to_find: &[u8; 32],
    ) -> Option<Vec<([u8; 32], bool)>> {
        let mut hashes: Vec<[u8; 32]> = Vec::new();
        for tx in &self.txn {
            hashes.push(tx.hash());
        }
        let merkle_tree = MerkleTree::new(&hashes);
        merkle_tree.merkle_proof_of_inclusion(*tx_id_to_find)
    }

    /// Devuelve un string que representa el hash del bloque en hexadecimal,
    /// En el formato que se usan los exploradores web como
    /// https://blockstream.info/testnet/ para mostrar bloques
    pub fn hex_hash(&self) -> String {
        self.block_header.hex_hash()
    }

    /// Devuelve un string que representa el hash del merkle root en hexadecimal,
    pub fn hex_merkle_root_hash(&self) -> String {
        self.block_header.hex_merkle_root_hash()
    }

    /// Notifica si el bloque contiene una transacción que se encontraba pendiente.
    /// Revisa las transacciones del bloque y las compara con las transacciones pendientes
    /// De las cuentas
    pub fn contains_pending_tx(
        &self,
        log_sender: &LogSender,
        ui_sender: &Option<glib::Sender<UIEvent>>,
        accounts: Arc<RwLock<Arc<RwLock<Vec<Account>>>>>,
    ) -> Result<(), NodeCustomErrors> {
        for tx in &self.txn {
            for account in &*accounts
                .read()
                .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                .read()
                .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
            {
                if account
                    .pending_transactions
                    .read()
                    .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                    .contains(tx)
                {
                    println!(
                        "EL BLOQUE {} \nCONTIENE LA TRANSACCION {} CONFIRMADA \nDE LA CUENTA {}\n",
                        self.hex_hash(),
                        tx.hex_hash(),
                        account.address
                    );
                    let pending_transaction_index = account
                        .pending_transactions
                        .read()
                        .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                        .iter()
                        .position(|pending_tx| pending_tx.hash() == tx.hash());
                    if let Some(pending_transaction_index) = pending_transaction_index {
                        let confirmed_tx = account
                            .pending_transactions
                            .write()
                            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                            .remove(pending_transaction_index);
                        account
                            .confirmed_transactions
                            .write()
                            .map_err(|err| NodeCustomErrors::LockError(err.to_string()))?
                            .push(confirmed_tx.clone());
                        write_in_log(
                            &log_sender.info_log_sender,
                            format!(
                                "CUENTA: {}: SE CONFIRMA NUEVA TRANSACCION {} EN BLOQUE --{}--",
                                account.address,
                                confirmed_tx.hex_hash(),
                                self.hex_hash()
                            )
                            .as_str(),
                        );
                        send_event_to_ui(
                            ui_sender,
                            UIEvent::ShowConfirmedTransaction(
                                self.clone(),
                                account.clone(),
                                tx.clone(),
                            ),
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Devuelve el hash del bloque
    pub fn hash(&self) -> [u8; 32] {
        self.block_header.hash()
    }

    /// Devuelve un string que representa el timestamp del bloque en formato UTC
    pub fn utc_time(&self) -> String {
        self.block_header.local_time()
    }

    /// Devuelve la altura del bloque
    pub fn get_height(&self) -> u32 {
        self.txn[0].get_height()
    }
}

#[cfg(test)]
mod test {
    use crate::{
        blocks::{block_header::BlockHeader, utils_block::concatenate_and_hash},
        compact_size_uint::CompactSizeUint,
        transactions::{
            outpoint::Outpoint, script::sig_script::SigScript, transaction::Transaction,
            tx_in::TxIn, tx_out::TxOut,
        },
    };
    use std::{error::Error, io, vec};

    use super::Block;

    /// Convierte el str recibido en hexadecimal, a bytes
    fn string_to_bytes(input: &str) -> Result<[u8; 32], Box<dyn Error>> {
        if input.len() != 64 {
            return Err(Box::new(std::io::Error::new(
                io::ErrorKind::Other,
                "El string recibido es inválido. No tiene el largo correcto",
            )));
        }

        let mut result = [0; 32];
        for i in 0..32 {
            let byte_str = &input[i * 2..i * 2 + 2];
            result[i] = u8::from_str_radix(byte_str, 16)?;
        }

        Ok(result)
    }
    /// Convierte los bytes a hexadecimal.
    pub fn bytes_to_hex_string(bytes: &[u8]) -> String {
        let hex_chars: Vec<String> = bytes.iter().map(|byte| format!("{:02x}", byte)).collect();

        hex_chars.join("")
    }

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

    fn crear_transaccion(
        version: i32,
        tx_in_count: u128,
        tx_out_count: u128,
        lock_time: u32,
    ) -> Transaction {
        //contenedor de bytes
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
        transaction
    }

    #[test]
    fn test_unmarshaling_del_bloque_genera_block_header_esperado() -> Result<(), &'static str> {
        let mut bytes_to_read: Vec<u8> = Vec::new();
        let block_header: BlockHeader = BlockHeader {
            version: (0x30201000),
            previous_block_header_hash: ([1; 32]),
            merkle_root_hash: ([2; 32]),
            time: (0x90807060),
            n_bits: (0x04030201),
            nonce: (0x30),
        };
        block_header.marshalling(&mut bytes_to_read);
        let txn_count_bytes: CompactSizeUint = CompactSizeUint::new(1);
        let txn_count: Vec<u8> = txn_count_bytes.marshalling();
        bytes_to_read.extend_from_slice(&txn_count);
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let tx: Transaction = crear_transaccion(version, tx_in_count, tx_out_count, lock_time);
        tx.marshalling(&mut bytes_to_read);
        let mut offset: usize = 0;
        let block: Block = Block::unmarshalling(&bytes_to_read, &mut offset)?;
        assert_eq!(block.block_header, block_header);
        Ok(())
    }

    #[test]
    fn test_unmarshaling_del_bloque_genera_txn_count_esperado() -> Result<(), &'static str> {
        let mut bytes_to_read: Vec<u8> = Vec::new();
        let block_header: BlockHeader = BlockHeader {
            version: (0x30201000),
            previous_block_header_hash: ([1; 32]),
            merkle_root_hash: ([2; 32]),
            time: (0x90807060),
            n_bits: (0x04030201),
            nonce: (0x30),
        };
        block_header.marshalling(&mut bytes_to_read);
        let txn_count_bytes: CompactSizeUint = CompactSizeUint::new(1);
        let txn_count: Vec<u8> = txn_count_bytes.marshalling();
        bytes_to_read.extend_from_slice(&txn_count);
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        let tx: Transaction = crear_transaccion(version, tx_in_count, tx_out_count, lock_time);
        tx.marshalling(&mut bytes_to_read);
        let mut offset: usize = 0;
        let block: Block = Block::unmarshalling(&bytes_to_read, &mut offset)?;
        assert_eq!(block.txn_count, txn_count_bytes);
        Ok(())
    }

    #[test]
    fn test_unmarshaling_del_bloque_genera_transaction_esperada() -> Result<(), &'static str> {
        let mut bytes_to_read: Vec<u8> = Vec::new();
        let block_header: BlockHeader = BlockHeader {
            version: (0x30201000),
            previous_block_header_hash: ([1; 32]),
            merkle_root_hash: ([2; 32]),
            time: (0x90807060),
            n_bits: (0x04030201),
            nonce: (0x30),
        };
        block_header.marshalling(&mut bytes_to_read);
        let txn_count_bytes: CompactSizeUint = CompactSizeUint::new(1);
        let txn_count: Vec<u8> = txn_count_bytes.marshalling();
        bytes_to_read.extend_from_slice(&txn_count);
        let tx_in_count: u128 = 1;
        let tx_out_count: u128 = 1;
        let version: i32 = 100;
        let lock_time: u32 = 3;
        let tx: Transaction = crear_transaccion(version, tx_in_count, tx_out_count, lock_time);
        tx.marshalling(&mut bytes_to_read);
        let mut offset: usize = 0;
        let block: Block = Block::unmarshalling(&bytes_to_read, &mut offset)?;
        assert_eq!(block.txn[0], tx);
        Ok(())
    }

    #[test]
    fn test_merkle_root_de_un_bloque_con_2_transacciones_se_genera_correctamente() {
        let block_header: BlockHeader = BlockHeader {
            version: (0x30201000),
            previous_block_header_hash: ([1; 32]),
            merkle_root_hash: ([2; 32]),
            time: (0x90807060),
            n_bits: (0x04030201),
            nonce: (0x30),
        };
        let txn_count_bytes: CompactSizeUint = CompactSizeUint::new(2);
        let mut txn: Vec<Transaction> = Vec::new();
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        txn.push(crear_transaccion(
            version,
            tx_in_count,
            tx_out_count,
            lock_time,
        ));
        let tx_in_count: u128 = 5;
        let tx_out_count: u128 = 3;
        let version: i32 = 34;
        let lock_time: u32 = 3;
        txn.push(crear_transaccion(
            version,
            tx_in_count,
            tx_out_count,
            lock_time,
        ));
        let first_hash: [u8; 32] = txn[0].hash();
        let second_hash: [u8; 32] = txn[1].hash();
        let expected_hash = concatenate_and_hash(first_hash, second_hash);
        let block: Block = Block::new(block_header, txn_count_bytes, txn);
        assert_eq!(block.generate_merkle_root(), expected_hash);
    }

    #[test]
    fn test_merkle_root_de_un_bloque_con_3_transacciones_se_genera_correctamente() {
        let block_header: BlockHeader = BlockHeader {
            version: (0x30201000),
            previous_block_header_hash: ([1; 32]),
            merkle_root_hash: ([2; 32]),
            time: (0x90807060),
            n_bits: (0x04030201),
            nonce: (0x30),
        };
        let txn_count_bytes: CompactSizeUint = CompactSizeUint::new(2);
        let mut txn: Vec<Transaction> = Vec::new();
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 3;
        txn.push(crear_transaccion(
            version,
            tx_in_count,
            tx_out_count,
            lock_time,
        ));
        let tx_in_count: u128 = 9;
        let tx_out_count: u128 = 3;
        let version: i32 = -34;
        let lock_time: u32 = 67;
        txn.push(crear_transaccion(
            version,
            tx_in_count,
            tx_out_count,
            lock_time,
        ));
        let tx_in_count: u128 = 4;
        let tx_out_count: u128 = 2;
        let version: i32 = 39;
        let lock_time: u32 = 3;
        txn.push(crear_transaccion(
            version,
            tx_in_count,
            tx_out_count,
            lock_time,
        ));
        let first_hash: [u8; 32] = txn[0].hash();
        let second_hash: [u8; 32] = txn[1].hash();
        let third_hash: [u8; 32] = txn[2].hash();
        let expected_hash_1 = concatenate_and_hash(first_hash, second_hash);
        let expected_hash_2 = concatenate_and_hash(third_hash, third_hash);
        let expected_hash_final = concatenate_and_hash(expected_hash_1, expected_hash_2);
        let block: Block = Block::new(block_header, txn_count_bytes, txn);
        assert_eq!(block.generate_merkle_root(), expected_hash_final);
    }

    #[test]
    fn test_generacion_correcta_del_merkle_root_hash_de_bloque_de_la_mainnet(
    ) -> Result<(), Box<dyn Error>> {
        // bloque 00000000000000127a638dfa7b517f1045217884cb986ab8f653b8be0ab37447
        // esos reverse son parapasar el verdadero id ya que en la pagina los hashes
        // estan cargados en LE
        // link a la pagina : https://tbtc.bitaps.com/00000000000000127a638dfa7b517f1045217884cb986ab8f653b8be0ab37447
        let mut transactions: Vec<[u8; 32]> = Vec::new();
        let mut coinbase =
            string_to_bytes("129f32d171b2a0c4ad5fd21f7504ae483845d311214f79eb927db49dfb28b838")?;
        coinbase.reverse();
        transactions.push(coinbase);
        let mut tx_1 =
            string_to_bytes("aefeb6fb10f2f6a63a3cd4f70f1b7f8b193881a10ae5832a595e938d1630f1b9")?;
        tx_1.reverse();
        transactions.push(tx_1);
        let mut tx_2 =
            string_to_bytes("4b0d8fd869e252803909aed9642bc8af28ebd18f2c4045b9b41679eda0ff79dd")?;
        tx_2.reverse();
        transactions.push(tx_2);
        let mut tx_3 =
            string_to_bytes("dbd558c896afe59a6dce2dc26bc32f4679b336ff0b1c0f2f8aaee846c5732333")?;
        tx_3.reverse();
        transactions.push(tx_3);
        let mut tx_4 =
            string_to_bytes("88030de1d5f1b023893f8258df1796863756d99eef5c91a5528362f73497ac51")?;
        tx_4.reverse();
        transactions.push(tx_4);
        let mut merkle_root = Block::recursive_generation_merkle_root(transactions);
        merkle_root.reverse();
        let hash_generated = bytes_to_hex_string(&merkle_root);
        let hash_expected = "bc689ae06069c1381eb92aabef250bb576d8aac8aedec9b7533a37351b6dedf8";
        assert_eq!(hash_generated, hash_expected);
        Ok(())
    }
}
