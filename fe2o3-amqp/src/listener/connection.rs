use tokio::io::{AsyncRead, AsyncWrite};

use crate::acceptor::{ConnectionAcceptor, self};

/// Listener for incoming connection
/// 
/// # Example
/// 
/// ```rust
/// let acceptor = ConnectionAcceptor::new("example-listener");
/// let listener = ConnectionListener::from(acceptor);
/// TODO
/// ```
#[derive(Debug)]
pub struct ConnectionListener<Io, Tls, Sasl> {
    io: Io,
    acceptor: ConnectionAcceptor<Tls, Sasl>,
}

impl<Tls, Sasl> From<ConnectionAcceptor<Tls, Sasl>> for ConnectionListener<(), Tls, Sasl> {
    fn from(acceptor: ConnectionAcceptor<Tls, Sasl>) -> Self {
        Self {
            io: (),
            acceptor,
        }
    }
}

impl<Tls, Sasl> ConnectionListener<(), Tls, Sasl> {
    /// Binds the listener to a stream
    pub fn bind<Io>(self, io: Io) -> ConnectionListener<Io, Tls, Sasl> {
        ConnectionListener { io, acceptor: self.acceptor }
    }
}

impl<Io, Tls, Sasl> ConnectionListener<Io, Tls, Sasl> {
    /// Unbinds the listener from the underlying IO
    pub fn unbind(self) -> (ConnectionListener<(), Tls, Sasl>, Io) {
        let io = self.io;
        let listener = ConnectionListener {
            io: (),
            acceptor: self.acceptor
        };
        (listener, io)
    }
}