//! Session Listener

// /// Listener for incoming session
// #[derive(Debug)]
// pub struct SessionListener {}

use async_trait::async_trait;
use fe2o3_amqp_types::{
    definitions::{self, AmqpError, SessionError},
    performatives::{Attach, Begin, Detach, Disposition, End, Flow, Transfer},
    states::SessionState,
};
use futures_util::{Sink};
use tokio::sync::mpsc;
use tokio_util::sync::PollSender;

use crate::{
    control::SessionControl,
    endpoint::{self, LinkFlow},
    session::{
        self, engine::SessionEngine, frame::SessionFrame, SessionHandle,
        DEFAULT_SESSION_CONTROL_BUFFER_SIZE, Error,
    },
    Payload, util::Initialized, link::{LinkHandle, LinkFrame},
};

use super::{ListenerConnectionHandle, builder::Builder};

type SessionBuilder = crate::session::Builder;

/// Type alias for listener session handle
pub type ListenerSessionHandle = SessionHandle<mpsc::Receiver<Attach>>;

// /// An acceptor for incoming session
// #[derive(Debug)]
// pub struct SessionAcceptor {
//     /// The transfer-id of the first transfer id the sender will send
//     pub next_outgoing_id: TransferNumber,

//     /// The initial incoming-window of the sender
//     pub incoming_window: TransferNumber,

//     /// The initial outgoing-window of the sender
//     pub outgoing_window: TransferNumber,

//     /// The maximum handle value that can be used on the session
//     pub handle_max: Handle,

//     /// The extension capabilities the sender supports
//     pub offered_capabilities: Option<Vec<Symbol>>,

//     /// The extension capabilities the sender can use if the receiver supports them
//     pub desired_capabilities: Option<Vec<Symbol>>,

//     /// Session properties
//     pub properties: Option<Fields>,

//     /// Buffer size of the underlying [`tokio::sync::mpsc::channel`]
//     /// that are used by links attached to the session
//     pub buffer_size: usize,
// }

/// An acceptor for incoming session
#[derive(Debug)]
pub struct SessionAcceptor(pub(crate) SessionBuilder);

impl Default for SessionAcceptor {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl SessionAcceptor {
    /// Creates a new builder for [`SessionAcceptor`]
    pub fn builder() -> Builder<Self, Initialized> {
        Builder::<Self, Initialized>::new()
    }

    /// Accepts an incoming session
    pub async fn accept(
        &self,
        connection: &mut ListenerConnectionHandle,
    ) -> Result<ListenerSessionHandle, Error> {
        let local_state = SessionState::Unmapped;
        let (session_control_tx, session_control_rx) =
            mpsc::channel::<SessionControl>(DEFAULT_SESSION_CONTROL_BUFFER_SIZE);
        let (incoming_tx, incoming_rx) = mpsc::channel(self.0.buffer_size);
        let (outgoing_tx, outgoing_rx) = mpsc::channel(self.0.buffer_size);
        let (link_listener_tx, link_listener_rx) = mpsc::channel(self.0.buffer_size);

        // create session in connection::Engine
        let (outgoing_channel, session_id) = connection.allocate_session(incoming_tx).await?; // AllocSessionError
        let session = self.0.clone()
            .into_session(session_control_tx.clone(), outgoing_channel, local_state);
        let listener_session = ListenerSession {
            session,
            link_listener: link_listener_tx,
        };
        let engine = SessionEngine::begin(
            connection.control.clone(),
            listener_session,
            session_id,
            session_control_rx,
            incoming_rx,
            PollSender::new(connection.outgoing.clone()),
            outgoing_rx,
        )
        .await?;
        let engine_handle = engine.spawn();
        let handle = SessionHandle {
            control: session_control_tx,
            engine_handle,
            outgoing: outgoing_tx,
            link_listener: link_listener_rx,
        };
        Ok(handle)
    }
}

// impl Session {
//     /// Accepts a remotely initiated session with default configuration
//     pub async fn accept(
//         connection: &mut ListenerConnectionHandle,
//     ) -> Result<ListenerSessionHandle, SessionError> {
//         Session::builder().accept_inner(connection).await
//     }
// }

// impl SessionBuilder {
//     /// Accepts a remotely initiated session
//     pub async fn accept(
//         &self,
//         connection: &mut ListenerConnectionHandle,
//     ) -> Result<ListenerSessionHandle, SessionError> {
//         self.clone().accept_inner(connection).await
//     }

//     async fn accept_inner(
//         self,
//         connection: &mut ListenerConnectionHandle,
//     ) -> Result<ListenerSessionHandle, SessionError> {
//         let local_state = SessionState::Unmapped;
//         let (session_control_tx, session_control_rx) =
//             mpsc::channel::<SessionControl>(DEFAULT_SESSION_CONTROL_BUFFER_SIZE);
//         let (incoming_tx, incoming_rx) = mpsc::channel(self.buffer_size);
//         let (outgoing_tx, outgoing_rx) = mpsc::channel(self.buffer_size);
//         let (link_listener_tx, link_listener_rx) = mpsc::channel(self.buffer_size);

//         // create session in connection::Engine
//         let (outgoing_channel, session_id) = connection.allocate_session(incoming_tx).await?; // AllocSessionError
//         let session = self.into_session(session_control_tx.clone(), outgoing_channel, local_state);
//         let listener_session = ListenerSession {
//             session,
//             link_listener: link_listener_tx,
//         };
//         let engine = SessionEngine::begin(
//             connection.control.clone(),
//             listener_session,
//             session_id,
//             session_control_rx,
//             incoming_rx,
//             PollSender::new(connection.outgoing.clone()),
//             outgoing_rx,
//         )
//         .await?;
//         let engine_handle = engine.spawn();
//         let handle = SessionHandle {
//             control: session_control_tx,
//             engine_handle,
//             outgoing: outgoing_tx,
//             link_listener: link_listener_rx,
//         };
//         Ok(handle)
//     }
// }

/// A session on the listener side
#[derive(Debug)]
pub struct ListenerSession {
    pub(crate) session: session::Session,
    pub(crate) link_listener: mpsc::Sender<Attach>,
}

#[async_trait]
impl endpoint::Session for ListenerSession {
    type AllocError = <session::Session as endpoint::Session>::AllocError;

    type Error = <session::Session as endpoint::Session>::Error;

    type State = <session::Session as endpoint::Session>::State;

    type LinkHandle = <session::Session as endpoint::Session>::LinkHandle;

    fn local_state(&self) -> &Self::State {
        self.session.local_state()
    }

    fn local_state_mut(&mut self) -> &mut Self::State {
        self.session.local_state_mut()
    }

    fn outgoing_channel(&self) -> u16 {
        self.session.outgoing_channel()
    }

    fn allocate_link(
        &mut self,
        link_name: String,
        link_handle: Self::LinkHandle,
    ) -> Result<fe2o3_amqp_types::definitions::Handle, Self::AllocError> {
        self.session.allocate_link(link_name, link_handle)
    }

    fn deallocate_link(&mut self, link_name: String) {
        self.session.deallocate_link(link_name)
    }

    fn on_incoming_begin(&mut self, channel: u16, begin: Begin) -> Result<(), Self::Error> {
        self.session.on_incoming_begin(channel, begin)
    }

    async fn on_incoming_attach(
        &mut self,
        _channel: u16,
        attach: Attach,
    ) -> Result<(), Self::Error> {
        // Look up link handle by link name
        match self.session.link_by_name.get(&attach.name) {
            Some(output_handle) => match self.session.local_links.get_mut(output_handle.0 as usize) {
                Some(link) => {
                    // Only Sender need to update the receiver settle mode
                    // because the sender needs to echo a disposition if
                    // rcv-settle-mode is 1
                    if let LinkHandle::Sender {
                        receiver_settle_mode,
                        ..
                    } = link
                    {
                        *receiver_settle_mode = attach.rcv_settle_mode.clone();
                    }

                    let input_handle = attach.handle.clone(); // handle is just a wrapper around u32
                    self.session.link_by_input_handle
                        .insert(input_handle, output_handle.clone());
                    match link.send(LinkFrame::Attach(attach)).await {
                        Ok(_) => Ok(()),
                        Err(_) => {
                            // TODO: how should this error be handled?
                            // End with UnattachedHandle?
                            return Err(Error::session_error(SessionError::UnattachedHandle, None));
                            // End session with unattached handle?
                        }
                    }
                }
                None => {
                    // TODO: Resuming link
                    return Err(Error::amqp_error(
                        AmqpError::NotImplemented,
                        "Link resumption is not supported yet".to_string()
                    ))
                }
            },
            None => {
                // If no such terminus exists, the application MAY
                // choose to create one using the properties supplied by the 
                // remote link endpoint. The link endpoint is then mapped
                // to an unused handle, and an attach frame is issued carrying 
                // the state of the newly created endpoint.
                self.link_listener.send(attach).await
                    .map_err(|_| {
                        // SessionHandle must have been dropped
                        Error::amqp_error(
                            AmqpError::IllegalState,
                            Some("Listener session handle must have been dropped".to_string()),
                        )
                    })?;
                Ok(())
            },
        }
    }

    async fn on_incoming_flow(&mut self, channel: u16, flow: Flow) -> Result<(), Self::Error> {
        self.session.on_incoming_flow(channel, flow).await
    }

    async fn on_incoming_transfer(
        &mut self,
        channel: u16,
        transfer: Transfer,
        payload: Payload,
    ) -> Result<(), Self::Error> {
        self.session
            .on_incoming_transfer(channel, transfer, payload)
            .await
    }

    async fn on_incoming_disposition(
        &mut self,
        channel: u16,
        disposition: Disposition,
    ) -> Result<(), Self::Error> {
        self.session
            .on_incoming_disposition(channel, disposition)
            .await
    }

    async fn on_incoming_detach(
        &mut self,
        channel: u16,
        detach: Detach,
    ) -> Result<(), Self::Error> {
        self.session.on_incoming_detach(channel, detach).await
    }

    async fn on_incoming_end(&mut self, channel: u16, end: End) -> Result<(), Self::Error> {
        self.session.on_incoming_end(channel, end).await
    }

    // Handling SessionFrames
    async fn send_begin<W>(&mut self, writer: &mut W) -> Result<(), Self::Error>
    where
        W: Sink<SessionFrame> + Send + Unpin,
    {
        self.session.send_begin(writer).await
    }

    async fn send_end<W>(
        &mut self,
        writer: &mut W,
        error: Option<definitions::Error>,
    ) -> Result<(), Self::Error>
    where
        W: Sink<SessionFrame> + Send + Unpin,
    {
        self.session.send_end(writer, error).await
    }

    // Intercepting LinkFrames
    fn on_outgoing_attach(&mut self, attach: Attach) -> Result<SessionFrame, Self::Error> {
        self.session.on_outgoing_attach(attach)
    }

    fn on_outgoing_flow(&mut self, flow: LinkFlow) -> Result<SessionFrame, Self::Error> {
        self.session.on_outgoing_flow(flow)
    }

    fn on_outgoing_transfer(
        &mut self,
        transfer: Transfer,
        payload: Payload,
    ) -> Result<SessionFrame, Self::Error> {
        self.session.on_outgoing_transfer(transfer, payload)
    }

    fn on_outgoing_disposition(
        &mut self,
        disposition: Disposition,
    ) -> Result<SessionFrame, Self::Error> {
        self.session.on_outgoing_disposition(disposition)
    }

    fn on_outgoing_detach(&mut self, detach: Detach) -> Result<SessionFrame, Self::Error> {
        self.session.on_outgoing_detach(detach)
    }
}
