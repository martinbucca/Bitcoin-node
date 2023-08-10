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

#[derive(Debug, Clone)]
/// Stores the 3 types of LogSender used in the program
pub struct LogSender {
    pub info_log_sender: LogFileSender,
    pub error_log_sender: LogFileSender,
    pub message_log_sender: LogFileSender,
}
#[derive(Debug)]
/// Stores the 3 types of JoinHandle used in the program
pub struct LogSenderHandles {
    pub info_handler: JoinHandle<()>,
    pub error_handler: JoinHandle<()>,
    pub message_handler: JoinHandle<()>,
}

/// Initializes the loggers.
/// Receives the file path of each one.
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

/// Closes the loggers
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

/// Given the endpoint to write through the channel and a JoinHandle of the thread that is writing in the log file,
/// prints that it is going to close the file, closes the channel endpoint and joins the thread to finish. Returns
/// error if the message can not be sent through the channel or the thread can not be joined correctly.
fn shutdown_logger(tx: LogFileSender, handler: JoinHandle<()>) -> Result<(), NodeCustomErrors> {
    tx.send(format!("Closing log \n\n{}", FINAL_LOG_LINE))
        .map_err(|err| NodeCustomErrors::WritingInFileError(err.to_string()))?;
    drop(tx);
    handler
        .join()
        .map_err(|err| NodeCustomErrors::ThreadJoinError(format!("{:?}", err)))?;
    Ok(())
}

/// Prints the message in the logFile received.
pub fn write_in_log(log_sender: &LogFileSender, msg: &str) {
    if let Err(err) = log_sender.send(msg.to_string()) {
        println!(
            "Error trying to write {} in the log file!, error: {}\n",
            msg, err
        );
    };
}

/// Receives a String with the name of the log file and is in charge of opening/creating the file and creating a thread that will be constantly listening
/// for the channel logs to write in the log file. Writes the current date as soon as it opens the file. In case of an error
/// it prints it to the console and keeps listening. Returns the endpoint to send through the channel and the JoinHandle of the thread in a tuple.
pub fn create_logger(
    log_file: &String,
    config: &Arc<Config>,
) -> Result<(LogFileSender, JoinHandle<()>), NodeCustomErrors> {
    let (tx, rx): (Sender<String>, Receiver<String>) = channel();
    let mut file = open_log_file(config, log_file)?;
    let date = get_initial_date_format();
    if let Err(err) = writeln!(file, "{}", date) {
        println!(
            "Error writing the logginf date: {}, {}",
            date,
            NodeCustomErrors::WritingInFileError(err.to_string())
        );
    }
    let handle = thread::spawn(move || {
        for log in rx {
            let date = get_date_as_string();
            if let Err(err) = writeln!(file, "{}: {}", date, log) {
                println!(
                    "Error {} trying to write in the log: {}",
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

/// Opens the file where it will print the log.
fn open_log_file(config: &Arc<Config>, log_file: &String) -> Result<File, NodeCustomErrors> {
    let logs_dir = PathBuf::from(config.logs_folder_path.clone());
    let log_path = logs_dir.join(log_file);
    // Creates the "logs" directory if it does not exist
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

/// Returns a string with the current date formatted
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

/// Returns a string with the current time formatted
fn get_date_as_string() -> String {
    format!(
        "{}:{}:{:02}",
        Local::now().hour(),
        Local::now().minute(),
        Local::now().second()
    )
}
