use chrono::{Datelike, Local, Timelike};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
};

use crate::{config::Config, custom_errors::NodeCustomErrors};

const CENTER_DATE_LINE: &str = "-------------------------------------------";
const FINAL_LOG_LINE: &str = "-----------------------------------------------------------------------------------------------------------------------------";

type LogFileSender = Sender<String>;

/// Almacena los 3 tipos de LogSender que se utilizan en el programa
#[derive(Debug, Clone)]
pub struct LogSender {
    pub info_log_sender: LogFileSender,
    pub error_log_sender: LogFileSender,
    pub message_log_sender: LogFileSender,
}
/// Almacena los 3 tipos de JoinHandle que se utilizan en el programa
#[derive(Debug)]
pub struct LogSenderHandles {
    pub info_handler: JoinHandle<()>,
    pub error_handler: JoinHandle<()>,
    pub message_handler: JoinHandle<()>,
}

/// Inicializa los loggers.
/// Recibe el file path de cada uno
pub fn set_up_loggers(
    config: &Arc<Config>,
) -> Result<(LogSender, LogSenderHandles), NodeCustomErrors> {
    let (info_log_sender, info_handler) = create_logger(&config.info_log_path, config)?;
    let (error_log_sender, error_handler) = create_logger(&config.error_log_path, config)?;
    let (message_log_sender, message_handler) = create_logger(&config.message_log_path, config)?;
    let log_sender = LogSender {
        info_log_sender,
        error_log_sender,
        message_log_sender,
    };
    let log_sender_handles = LogSenderHandles {
        info_handler,
        error_handler,
        message_handler,
    };
    Ok((log_sender, log_sender_handles))
}

/// Cierra los loggers
pub fn shutdown_loggers(
    log_sender: LogSender,
    log_sender_handles: LogSenderHandles,
) -> Result<(), NodeCustomErrors> {
    shutdown_logger(log_sender.info_log_sender, log_sender_handles.info_handler)?;
    shutdown_logger(
        log_sender.error_log_sender,
        log_sender_handles.error_handler,
    )?;
    shutdown_logger(
        log_sender.message_log_sender,
        log_sender_handles.message_handler,
    )?;
    Ok(())
}

/// Dado el extremo para escribir por el channel y un JoinHandle del thread que esta escribiendo en el archivo log,
/// imprime que va a cerrar el archivo, cierra el extremo del channel y le hace join al thread para que termine. Devuelve
/// error en caso de que no se pueda mandar el mensaje por el channel o no se pueda hacer join correctamente al thread
fn shutdown_logger(tx: LogFileSender, handler: JoinHandle<()>) -> Result<(), NodeCustomErrors> {
    tx.send(format!("Closing log \n\n{}", FINAL_LOG_LINE))
        .map_err(|err| NodeCustomErrors::WritingInFileError(err.to_string()))?;
    drop(tx);
    handler
        .join()
        .map_err(|err| NodeCustomErrors::ThreadJoinError(format!("{:?}", err)))?;
    Ok(())
}

/// Imprime el mensaje en el logFile recibido
pub fn write_in_log(log_sender: &LogFileSender, msg: &str) {
    if let Err(err) = log_sender.send(msg.to_string()) {
        println!(
            "Error al intentar escribir {} en el log!, error: {}\n",
            msg, err
        );
    };
}

/// Recibe un String con el nombre del archivo log y se encarga de abrir/crear el archivo y crear un thread que va a estar constantemente escuchando por el
/// channel logs para escribir en el archivo log. Escribe la fecha actual apenas abre el archivo. En caso de que haya un error
/// lo imprime por consola y sigue escuchando. Devuelve el extremo para mandar por el channel y el JoinHandle del thread en una tupla.
pub fn create_logger(
    log_file: &String,
    config: &Arc<Config>,
) -> Result<(LogFileSender, JoinHandle<()>), NodeCustomErrors> {
    let (tx, rx): (Sender<String>, Receiver<String>) = channel();
    let mut file = open_log_file(config, log_file)?;
    let date = get_initial_date_format();
    if let Err(err) = writeln!(file, "{}", date) {
        println!(
            "Error al escribir la fecha de logging: {}, {}",
            date,
            NodeCustomErrors::WritingInFileError(err.to_string())
        );
    }
    let handle = thread::spawn(move || {
        for log in rx {
            let date = get_date_as_string();
            if let Err(err) = writeln!(file, "{}: {}", date, log) {
                println!(
                    "Error {} al escribir en el log: {}",
                    NodeCustomErrors::WritingInFileError(err.to_string()),
                    log
                );
            };
        }
    });
    Ok((tx, handle))
}

/*
***************************************************************************
************************ AUXILIAR FUNCTIONS *******************************
***************************************************************************
*/

/// Abre el file donde va a imprimir el log
fn open_log_file(config: &Arc<Config>, log_file: &String) -> Result<File, NodeCustomErrors> {
    let logs_dir = PathBuf::from(config.logs_folder_path.clone());
    let log_path = logs_dir.join(log_file);
    // Crea el directorio "logs" si no existe
    if !logs_dir.exists() {
        std::fs::create_dir(&logs_dir)
            .map_err(|err| NodeCustomErrors::OpeningFileError(err.to_string()))?;
    }
    let log_open_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|err| NodeCustomErrors::OpeningFileError(err.to_string()))?;
    Ok(log_open_file)
}

/// Devuelve un string con la fecha actual formateada
fn get_initial_date_format() -> String {
    let local = Local::now();
    format!(
        "\n{} Actual date: {}-{}-{} Hour: {:02}:{:02}:{:02} {}\n",
        CENTER_DATE_LINE,
        local.day(),
        local.month(),
        local.year(),
        local.hour(),
        local.minute(),
        local.second(),
        CENTER_DATE_LINE
    )
}

/// Devuelve un string con la hora actual formateada
fn get_date_as_string() -> String {
    format!(
        "{}:{}:{:02}",
        Local::now().hour(),
        Local::now().minute(),
        Local::now().second()
    )
}
