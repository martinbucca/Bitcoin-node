use std::{error::Error, fmt};

#[derive(Debug, PartialEq, Eq, Clone)]
/// Representa los distintos errores genericos que pueden llegar a ocurrir
/// durante el programa
pub enum NodeCustomErrors {
    ThreadJoinError(String),
    LockError(String),
    ReadNodeError(String),
    WriteNodeError(String),
    CanNotRead(String),
    ThreadChannelError(String),
    UnmarshallingError(String),
    SocketError(String),
    HandshakeError(String),
    FirstBlockNotFoundError(String),
    InvalidHeaderError(String),
    ReadingFileError(String),
    WritingInFileError(String),
    ClosingFileError(String),
    OpeningFileError(String),
    ArgumentsError(String),
    BlockchainDownloadError(String),
    OtherError(String),
    UtxoError(String),
}

impl fmt::Display for NodeCustomErrors {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NodeCustomErrors::ThreadJoinError(msg) => {
                write!(f, "ThreadJoinError Error: {}", msg)
            }
            NodeCustomErrors::LockError(msg) => write!(f, "LockError Error: {}", msg),
            NodeCustomErrors::ReadNodeError(msg) => {
                write!(f, "Can not read from socket Error: {}", msg)
            }
            NodeCustomErrors::WriteNodeError(msg) => {
                write!(f, "Can not write in socket Error: {}", msg)
            }
            NodeCustomErrors::CanNotRead(msg) => {
                write!(f, "No more elements in list Error: {}", msg)
            }
            NodeCustomErrors::ThreadChannelError(msg) => {
                write!(f, "Can not send elements to channel Error: {}", msg)
            }
            NodeCustomErrors::UnmarshallingError(msg) => {
                write!(f, "Can not unmarshall bytes Error: {}", msg)
            }
            NodeCustomErrors::OtherError(msg) => {
                write!(f, "Error: {}", msg)
            }
            NodeCustomErrors::SocketError(msg) => {
                write!(f, "Socket-TcpStream Error: {}", msg)
            }
            NodeCustomErrors::HandshakeError(msg) => {
                write!(f, "HandShake Error: {}", msg)
            }
            NodeCustomErrors::FirstBlockNotFoundError(msg) => {
                write!(f, "FirstBlockNotFound Error: {}", msg)
            }
            NodeCustomErrors::InvalidHeaderError(msg) => {
                write!(f, "InvalidHeader Error: {}", msg)
            }
            NodeCustomErrors::ReadingFileError(msg) => {
                write!(f, "Failed to read file. File Error: {}", msg)
            }
            NodeCustomErrors::WritingInFileError(msg) => {
                write!(f, "Failed to write in file. File Error: {}", msg)
            }
            NodeCustomErrors::ClosingFileError(msg) => {
                write!(f, "Failed to close file. File Error: {}", msg)
            }
            NodeCustomErrors::OpeningFileError(msg) => {
                write!(f, "Failed to open file. File Error: {}", msg)
            }
            NodeCustomErrors::ArgumentsError(msg) => {
                write!(f, "Failed to parse arguments. Arguments Error: {}", msg)
            }
            NodeCustomErrors::BlockchainDownloadError(msg) => {
                write!(f, "Error during the Blockchain download: {}", msg)
            }
            NodeCustomErrors::UtxoError(msg) => {
                write!(f, "Error during the Utxo setup: {}", msg)
            }
        }
    }
}

impl Error for NodeCustomErrors {}
