use std::io;

use fe2o3_amqp_types::definitions::{AmqpError, ConnectionError};
use tokio::{sync::mpsc, task::JoinError};

use crate::{transport, error::EngineError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO Error {0:?}")]
    Io(#[from] io::Error),

    #[error("Idle timeout")]
    IdleTimeout,

    #[error(transparent)]
    UrlError(#[from] url::ParseError),

    #[error(transparent)]
    JoinError(JoinError),

    #[error("Exceeding channel-max")]
    ChannelMaxReached,

    #[error("AMQP error {:?}, {:?}", .condition, .description)]
    AmqpError {
        condition: AmqpError,
        description: Option<String>,
    },

    #[error("Connection error {:?}, {:?}", .condition, .description)]
    ConnectionError {
        condition: ConnectionError,
        description: Option<String>,
    },
}

impl<T> From<mpsc::error::SendError<T>> for Error 
where T: std::fmt::Debug 
{
    fn from(err: mpsc::error::SendError<T>) -> Self {
        Self::Io(io::Error::new(
            io::ErrorKind::Other,
            err.to_string()
        ))
    }
}

impl From<AmqpError> for Error {
    fn from(err: AmqpError) -> Self {
        Self::AmqpError {
            condition: err,
            description: None,
        }
    }
}

impl Error {
    pub fn amqp_error(
        condition: impl Into<AmqpError>,
        description: impl Into<Option<String>>,
    ) -> Self {
        Self::AmqpError {
            condition: condition.into(),
            description: description.into(),
        }
    }

    pub fn connection_error(
        condition: impl Into<ConnectionError>,
        description: impl Into<Option<String>>,
    ) -> Self {
        Self::ConnectionError {
            condition: condition.into(),
            description: description.into(),
        }
    }
}

impl From<transport::Error> for Error {
    fn from(err: transport::Error) -> Self {
        match err {
            transport::Error::Io(e) => Self::Io(e),
            transport::Error::IdleTimeout => Self::IdleTimeout,
            transport::Error::AmqpError {
                condition,
                description,
            } => Self::AmqpError {
                condition,
                description,
            },
            transport::Error::ConnectionError {
                condition,
                description,
            } => Self::ConnectionError {
                condition,
                description,
            },
        }
    }
}

impl From<Error> for EngineError {
    fn from(err: Error) -> Self {
        match err {
            Error::Io(e) => EngineError::Io(e),
            Error::IdleTimeout => EngineError::IdleTimeout,
            Error::UrlError(e) => EngineError::UrlError(e),
            Error::JoinError(e) => EngineError::JoinError(e),
            Error::AmqpError{condition, description: _} => {
                EngineError::AmqpError(condition)
            },
            Error::ConnectionError{condition, description: _} => {
                EngineError::ConnectionError(condition)
            },
            Error::ChannelMaxReached => EngineError::Message("Channel max reached")
        }
    }
}

/// Error associated with allocation of new session
#[derive(Debug, thiserror::Error)]
pub enum AllocSessionError {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error("Illegal local state")]
    IllegalState,

    #[error("Reached connection channel max")]
    ChannelMaxReached,
}

impl<T> From<mpsc::error::SendError<T>> for AllocSessionError 
where T: std::fmt::Debug 
{
    fn from(err: mpsc::error::SendError<T>) -> Self {
        Self::Io(io::Error::new(
            io::ErrorKind::Other,
            err.to_string()
        ))
    }
}

impl From<AllocSessionError> for EngineError {
    fn from(err: AllocSessionError) -> Self {
        match err {
            AllocSessionError::Io(e) => EngineError::Io(e),
            AllocSessionError::ChannelMaxReached => EngineError::Message("Channel max reached"),
            AllocSessionError::IllegalState => EngineError::AmqpError(AmqpError::IllegalState),
        }
    }
}