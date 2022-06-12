//! Session builder

use std::collections::BTreeMap;

use fe2o3_amqp_types::definitions::{Fields, Handle, TransferNumber};
use serde_amqp::primitives::Symbol;
use slab::Slab;
use tokio::sync::mpsc;
use tokio_util::sync::PollSender;

use crate::{
    connection::ConnectionHandle,
    control::SessionControl,
    endpoint::OutgoingChannel,
    session::{engine::SessionEngine, SessionState},
    util::Constant,
    Session, transaction::{TransactionManager, TransactionManagerBuilder},
};
use crate::link::LinkFrame;

use super::{Error, SessionHandle, DEFAULT_WINDOW};

pub(crate) const DEFAULT_SESSION_CONTROL_BUFFER_SIZE: usize = 128;
pub(crate) const DEFAULT_SESSION_MUX_BUFFER_SIZE: usize = u16::MAX as usize;

/// Builder for [`crate::Session`]
#[derive(Debug, Clone)]
pub struct Builder {
    /// The transfer-id of the first transfer id the sender will send
    pub next_outgoing_id: TransferNumber,

    /// The initial incoming-window of the sender
    pub incoming_window: TransferNumber,

    /// The initial outgoing-window of the sender
    pub outgoing_window: TransferNumber,

    /// The maximum handle value that can be used on the session
    pub handle_max: Handle,

    /// The extension capabilities the sender supports
    pub offered_capabilities: Option<Vec<Symbol>>,

    /// The extension capabilities the sender can use if the receiver supports them
    pub desired_capabilities: Option<Vec<Symbol>>,

    /// Session properties
    pub properties: Option<Fields>,

    /// Buffer size of the underlying [`tokio::sync::mpsc::channel`]
    /// that are used by links attached to the session
    pub buffer_size: usize,

    /// Transaction manager that manages incoming
    pub txn_manager_builder: TransactionManagerBuilder,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            next_outgoing_id: 0,
            incoming_window: DEFAULT_WINDOW,
            outgoing_window: DEFAULT_WINDOW,
            handle_max: Default::default(),
            offered_capabilities: None,
            desired_capabilities: None,
            properties: None,
            buffer_size: DEFAULT_SESSION_MUX_BUFFER_SIZE,
            txn_manager_builder: Default::default()
        }
    }
}

impl Builder {
    /// Creates a new builder for [`crate::Session`]
    pub fn new() -> Self {
        Self::default()
    }

    // #[cfg(not(feature = "transaction"))]
    // pub(crate) fn into_session(
    //     self,
    //     control: mpsc::Sender<SessionControl>,
    //     outgoing_channel: OutgoingChannel,
    //     local_state: SessionState,
    // ) -> Session {
    //     Session {
    //         control,
    //         outgoing_channel,
    //         local_state,
    //         initial_outgoing_id: Constant::new(self.next_outgoing_id),
    //         next_outgoing_id: self.next_outgoing_id,
    //         incoming_window: self.incoming_window,
    //         outgoing_window: self.outgoing_window,
    //         handle_max: self.handle_max,
    //         incoming_channel: None,
    //         next_incoming_id: 0,
    //         remote_incoming_window: 0,
    //         remote_outgoing_window: 0,
    //         offered_capabilities: self.offered_capabilities,
    //         desired_capabilities: self.desired_capabilities,
    //         properties: self.properties,

    //         link_name_by_output_handle: Slab::new(),
    //         link_by_name: BTreeMap::new(),
    //         link_by_input_handle: BTreeMap::new(),
    //         delivery_tag_by_id: BTreeMap::new(),
    //     }
    // }

    // #[cfg(feature = "transaction")]
    // pub(crate) fn into_session(
    //     self,
    //     control: mpsc::Sender<SessionControl>,
    //     outgoing_channel: OutgoingChannel,
    //     local_state: SessionState,
    //     txn_manager: mpsc::Sender<LinkFrame>,
    // ) -> Session {
    //     Session {
    //         control,
    //         outgoing_channel,
    //         local_state,
    //         initial_outgoing_id: Constant::new(self.next_outgoing_id),
    //         next_outgoing_id: self.next_outgoing_id,
    //         incoming_window: self.incoming_window,
    //         outgoing_window: self.outgoing_window,
    //         handle_max: self.handle_max,
    //         incoming_channel: None,
    //         next_incoming_id: 0,
    //         remote_incoming_window: 0,
    //         remote_outgoing_window: 0,
    //         offered_capabilities: self.offered_capabilities,
    //         desired_capabilities: self.desired_capabilities,
    //         properties: self.properties,

    //         link_name_by_output_handle: Slab::new(),
    //         link_by_name: BTreeMap::new(),
    //         link_by_input_handle: BTreeMap::new(),
    //         delivery_tag_by_id: BTreeMap::new(),
    //         txn_manager,
    //     }
    // }

    /// The transfer-id of the first transfer id the sender will send
    pub fn next_outgoing_id(mut self, value: TransferNumber) -> Self {
        self.next_outgoing_id = value;
        self
    }

    /// The initial incoming-window of the sender
    pub fn incoming_window(mut self, value: TransferNumber) -> Self {
        self.incoming_window = value;
        self
    }

    /// The initial outgoing-window of the sender
    pub fn outgoing_widnow(mut self, value: TransferNumber) -> Self {
        self.outgoing_window = value;
        self
    }

    /// The maximum handle value that can be used on the session
    pub fn handle_max(mut self, value: impl Into<Handle>) -> Self {
        self.handle_max = value.into();
        self
    }

    /// Add one extension capabilities the sender supports
    pub fn add_offered_capabilities(mut self, capability: impl Into<Symbol>) -> Self {
        match &mut self.offered_capabilities {
            Some(capabilities) => capabilities.push(capability.into()),
            None => self.offered_capabilities = Some(vec![capability.into()]),
        }
        self
    }

    /// Set the extension capabilities the sender supports
    pub fn set_offered_capabilities(mut self, capabilities: Vec<Symbol>) -> Self {
        self.offered_capabilities = Some(capabilities);
        self
    }

    /// Add one extension capabilities the sender can use if the receiver supports them
    pub fn add_desired_capabilities(mut self, capability: impl Into<Symbol>) -> Self {
        match &mut self.desired_capabilities {
            Some(capabilities) => capabilities.push(capability.into()),
            None => self.desired_capabilities = Some(vec![capability.into()]),
        }
        self
    }

    /// Set the extension capabilities the sender can use if the receiver supports them
    pub fn set_desired_capabilities(mut self, capabilities: Vec<Symbol>) -> Self {
        self.desired_capabilities = Some(capabilities);
        self
    }

    /// Session properties
    pub fn properties(mut self, properties: Fields) -> Self {
        self.properties = Some(properties);
        self
    }

    /// Buffer size of the underlying [`tokio::sync::mpsc::channel`]
    /// that are used by links attached to the session
    pub fn buffer_size(mut self, buffer_size: usize) -> Self {
        self.buffer_size = buffer_size;
        self
    }

    /// Set the transaction manager
    pub fn transaction_manager_builder(mut self, txn_manager_builder: TransactionManagerBuilder) -> Self {
        self.txn_manager_builder = txn_manager_builder;
        self
    }

    /// Begins a new session
    ///
    /// # Example
    ///
    /// ```rust, ignore
    /// let session = Session::builder()
    ///     .handle_max(128u32)
    ///     .begin(&mut connection)
    ///     .await.unwrap();
    /// ```
    ///
    pub async fn begin(
        self,
        connection: &mut ConnectionHandle<()>,
    ) -> Result<SessionHandle<()>, Error> {
        let local_state = SessionState::Unmapped;
        let (session_control_tx, session_control_rx) =
            mpsc::channel::<SessionControl>(DEFAULT_SESSION_CONTROL_BUFFER_SIZE);
        let (incoming_tx, incoming_rx) = mpsc::channel(self.buffer_size);
        let (outgoing_tx, outgoing_rx) = mpsc::channel(self.buffer_size);
        let (txn_manager_tx, txn_manager_rx) = mpsc::channel(self.buffer_size);

        // create session in connection::Engine
        let outgoing_channel = connection.allocate_session(incoming_tx).await?; // AllocSessionError

        let session = Session {
            control: session_control_tx.clone(),
            outgoing_channel,
            local_state,
            initial_outgoing_id: Constant::new(self.next_outgoing_id),
            next_outgoing_id: self.next_outgoing_id,
            incoming_window: self.incoming_window,
            outgoing_window: self.outgoing_window,
            handle_max: self.handle_max,
            incoming_channel: None,
            next_incoming_id: 0,
            remote_incoming_window: 0,
            remote_outgoing_window: 0,
            offered_capabilities: self.offered_capabilities,
            desired_capabilities: self.desired_capabilities,
            properties: self.properties,

            link_name_by_output_handle: Slab::new(),
            link_by_name: BTreeMap::new(),
            link_by_input_handle: BTreeMap::new(),
            delivery_tag_by_id: BTreeMap::new(),
            txn_manager: txn_manager_tx,
        };

        // let session = self.into_session(session_control_tx.clone(), outgoing_channel, local_state);
        let engine = SessionEngine::<crate::Session>::begin(
            connection.control.clone(),
            session,
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
            link_listener: (),
        };
        Ok(handle)
    }
}
