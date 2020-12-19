use crate::core::CoreMessage;
use crate::crypto::PublicKey;
use crate::messages::Block;
use crate::store::StoreError;
use ed25519_dalek::ed25519;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;

#[macro_export]
macro_rules! bail {
    ($e:expr) => {
        return Err($e);
    };
}

#[macro_export(local_inner_macros)]
macro_rules! ensure {
    ($cond:expr, $e:expr) => {
        if !($cond) {
            bail!($e);
        }
    };
}

pub type DiemResult<T> = Result<T, DiemError>;

#[derive(Error, Debug)]
pub enum DiemError {
    #[error("Serialization error. {0}")]
    SerializationError(Box<bincode::ErrorKind>),

    #[error("Network error. {0}")]
    NetworkError(std::io::Error),

    #[error("Store error. {0}")]
    StoreError(StoreError),

    #[error("Channel error. {0}")]
    ChannelError(String),

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Received an unexpected or late vote from {0:?}")]
    UnexpectedOrLateVote(PublicKey),

    #[error("Received more than one vote from {0:?}")]
    AuthorityReuse(PublicKey),

    #[error("Received vote from unknown authority {0:?}")]
    UnknownAuthority(PublicKey),

    #[error("Received QC without a quorum")]
    QCRequiresQuorum,

    #[error("Received unexpected message {0:?}")]
    UnexpectedMessage(CoreMessage),
}

impl From<Box<bincode::ErrorKind>> for DiemError {
    fn from(e: Box<bincode::ErrorKind>) -> Self {
        DiemError::SerializationError(e)
    }
}

impl From<std::io::Error> for DiemError {
    fn from(e: std::io::Error) -> Self {
        DiemError::NetworkError(e)
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for DiemError {
    fn from(e: tokio::sync::oneshot::error::RecvError) -> Self {
        DiemError::ChannelError(e.to_string())
    }
}

impl From<ed25519::Error> for DiemError {
    fn from(_e: ed25519::Error) -> Self {
        DiemError::InvalidSignature
    }
}

impl From<StoreError> for DiemError {
    fn from(e: StoreError) -> Self {
        DiemError::StoreError(e)
    }
}

impl From<SendError<CoreMessage>> for DiemError {
    fn from(e: SendError<CoreMessage>) -> Self {
        DiemError::ChannelError(format!("Core failed to send message to network: {}", e))
    }
}

impl From<SendError<Block>> for DiemError {
    fn from(e: SendError<Block>) -> Self {
        DiemError::ChannelError(format!(
            "Core failed to send message to commit channel: {}",
            e
        ))
    }
}
