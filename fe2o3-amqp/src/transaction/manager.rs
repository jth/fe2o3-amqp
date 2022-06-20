//! Manages incoming transaction on the resource side

use std::{
    collections::{BTreeMap, BTreeSet},
    thread::JoinHandle,
};

use async_trait::async_trait;
use fe2o3_amqp_types::{
    definitions,
    messaging::{TargetArchetype, DeliveryState},
    performatives::{Attach, Begin, Detach, Disposition, End, Flow, Transfer},
    primitives::Symbol,
    transaction::TransactionId,
};
use futures_util::Sink;
use serde_amqp::Value;
use tokio::sync::mpsc;

use crate::{
    endpoint::{self, IncomingChannel, InputHandle, LinkFlow, OutgoingChannel, OutputHandle},
    link::{target_archetype::VariantOfTargetArchetype, AttachError, LinkFrame, LinkRelay},
    session::{self, frame::SessionFrame, AllocLinkError},
    Payload, Session,
};

use super::{coordinator::ControlLinkAcceptor, frame::TransactionalWork};

#[async_trait]
pub(crate) trait HandleControlLink {
    type Error: Send;

    async fn on_incoming_control_attach(
        &mut self,
        channel: IncomingChannel,
        attach: Attach,
    ) -> Result<(), Self::Error>;

    fn on_incoming_control_detach(
        &mut self,
        channel: IncomingChannel,
        detach: Detach,
    ) -> Result<(), Self::Error>;
}

const TXN_ID_KEY: &str = "txn-id";

/// How an incoming transaction should be handled in a session
#[async_trait]
pub(crate) trait HandleTransactionalWork {
    type Error: Send;

    fn allocate_transaction_id(&mut self) -> Result<TransactionId, Self::Error>;

    fn commit_transaction(&mut self, txn_id: TransactionId) -> Result<(), Self::Error>;

    fn rollback_transaction(&mut self, txn_id: TransactionId) -> Result<(), Self::Error>;

    async fn on_incoming_txn_transfer(
        &mut self,
        channel: IncomingChannel,
        transfer: Transfer,
        payload: Payload,
    ) -> Result<(), Self::Error>;

    async fn on_incoming_txn_flow(
        &mut self,
        channel: IncomingChannel,
        flow: Flow,
    ) -> Result<(), Self::Error>;

    async fn on_incoming_txn_disposition(
        &mut self,
        channel: IncomingChannel,
        disposition: Disposition,
    ) -> Result<(), Self::Error>;

    fn on_outgoing_txn_transfer(&mut self, attach: Attach) -> Result<SessionFrame, Self::Error>;

    fn on_outgoing_txn_flow(&mut self, flow: LinkFlow) -> Result<SessionFrame, Self::Error>;

    fn on_outgoing_txn_disposition(
        &mut self,
        disposition: Disposition,
    ) -> Result<SessionFrame, Self::Error>;
}

/// Transaction manager
#[derive(Debug)]
pub(crate) struct TransactionManager {
    pub control_link_outgoing: mpsc::Sender<LinkFrame>,
    pub txn_id_source: u64,
    pub txns: BTreeMap<TransactionId, TransactionalWork>,
    pub control_link_acceptor: ControlLinkAcceptor,
    // pub coordinators: BTreeSet<JoinHandle<>>,
}

impl TransactionManager {
    pub(crate) fn new(
        control_link_outgoing: mpsc::Sender<LinkFrame>,
        control_link_acceptor: ControlLinkAcceptor,
    ) -> Self {
        Self {
            control_link_outgoing,
            txn_id_source: 0,
            txns: BTreeMap::new(),
            control_link_acceptor,
        }
    }
}

///
#[derive(Debug)]
pub(crate) struct TxnSession<S>
where
    S: endpoint::Session,
{
    pub(crate) session: S,
    pub(crate) txn_manager: TransactionManager,
}

#[async_trait]
impl<S> HandleControlLink for TxnSession<S>
where
    S: endpoint::Session<Error = session::Error> + endpoint::SessionExt + Send + Sync,
{
    type Error = S::Error;

    async fn on_incoming_control_attach(
        &mut self,
        channel: IncomingChannel,
        remote_attach: Attach,
    ) -> Result<(), Self::Error> {
        let coordinator = self
            .txn_manager
            .control_link_acceptor
            .accept_incoming_attach(
                remote_attach,
                self.session.control(),
                &self.txn_manager.control_link_outgoing,
            )
            .await
            .map_err(session::Error::CoordinatorAttachError)?;

        // TODO: store the joinhandle?
        let _ = tokio::spawn(coordinator.event_loop());
        Ok(())
    }

    fn on_incoming_control_detach(
        &mut self,
        channel: IncomingChannel,
        detach: Detach,
    ) -> Result<(), Self::Error> {
        todo!()
    }
}

#[async_trait]
impl<S> HandleTransactionalWork for TxnSession<S>
where
    S: endpoint::Session<Error = session::Error> + Send,
{
    type Error = S::Error;

    fn allocate_transaction_id(&mut self) -> Result<TransactionId, Self::Error> {
        todo!()
    }

    fn commit_transaction(&mut self, txn_id: TransactionId) -> Result<(), Self::Error> {
        todo!()
    }

    fn rollback_transaction(&mut self, txn_id: TransactionId) -> Result<(), Self::Error> {
        todo!()
    }

    fn on_incoming_txn_transfer<'life0, 'async_trait>(
        &'life0 mut self,
        channel: IncomingChannel,
        transfer: Transfer,
        payload: Payload,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<Output = Result<(), Self::Error>>
                + core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        todo!()
    }

    async fn on_incoming_txn_flow(
        &mut self,
        channel: IncomingChannel,
        flow: Flow,
    ) -> Result<(), Self::Error> {
        todo!()
    }

    async fn on_incoming_txn_disposition(
        &mut self,
        channel: IncomingChannel,
        disposition: Disposition,
    ) -> Result<(), Self::Error> {
        todo!()
    }

    fn on_outgoing_txn_transfer(&mut self, attach: Attach) -> Result<SessionFrame, Self::Error> {
        todo!()
    }

    fn on_outgoing_txn_flow(&mut self, flow: LinkFlow) -> Result<SessionFrame, Self::Error> {
        todo!()
    }

    fn on_outgoing_txn_disposition(
        &mut self,
        disposition: Disposition,
    ) -> Result<SessionFrame, Self::Error> {
        todo!()
    }
}

#[async_trait]
impl<S> endpoint::Session for TxnSession<S>
where
    S: endpoint::Session<Error = session::Error> + endpoint::SessionExt + Send + Sync,
{
    type AllocError = S::AllocError;

    type Error = S::Error;

    type State = S::State;

    fn local_state(&self) -> &Self::State {
        self.session.local_state()
    }
    fn local_state_mut(&mut self) -> &mut Self::State {
        self.session.local_state_mut()
    }
    fn outgoing_channel(&self) -> OutgoingChannel {
        self.session.outgoing_channel()
    }

    // Allocate new local handle for new Link
    fn allocate_link(
        &mut self,
        link_name: String,
        link_relay: Option<LinkRelay<()>>, // TODO: how to expose error at compile time?
    ) -> Result<OutputHandle, Self::AllocError> {
        self.session.allocate_link(link_name, link_relay)
    }

    fn allocate_incoming_link(
        &mut self,
        link_name: String,
        link_relay: LinkRelay<()>,
        input_handle: InputHandle,
    ) -> Result<OutputHandle, Self::AllocError> {
        self.session
            .allocate_incoming_link(link_name, link_relay, input_handle)
    }

    fn deallocate_link(&mut self, output_handle: OutputHandle) {
        self.session.deallocate_link(output_handle)
    }

    fn on_incoming_begin(
        &mut self,
        channel: IncomingChannel,
        begin: Begin,
    ) -> Result<(), Self::Error> {
        self.session.on_incoming_begin(channel, begin)
    }

    async fn on_incoming_attach(
        &mut self,
        channel: IncomingChannel,
        attach: Attach,
    ) -> Result<(), Self::Error> {
        match attach.target.as_ref().map(|t| t.is_coordinator()) {
            Some(true) => self.on_incoming_control_attach(channel, attach).await,
            Some(false) | None => self.session.on_incoming_attach(channel, attach).await,
        }
    }

    async fn on_incoming_flow(
        &mut self,
        channel: IncomingChannel,
        flow: Flow,
    ) -> Result<(), Self::Error> {
        match flow.properties.as_ref()
            .map(|fields| fields.contains_key(&Symbol::from(TXN_ID_KEY)))
        {
            Some(true) => self.on_incoming_txn_flow(channel, flow).await,
            Some(false)
            | None => self.session.on_incoming_flow(channel, flow).await,
        }
    }

    async fn on_incoming_transfer(
        &mut self,
        channel: IncomingChannel,
        transfer: Transfer,
        payload: Payload,
    ) -> Result<(), Self::Error> {
        match &transfer.state {
            Some(DeliveryState::TransactionalState(_)) => self.on_incoming_txn_transfer(channel, transfer, payload).await,
            Some(_)
            | None => self.session.on_incoming_transfer(channel, transfer, payload).await,
        }
    }

    async fn on_incoming_disposition(
        &mut self,
        channel: IncomingChannel,
        disposition: Disposition,
    ) -> Result<(), Self::Error> {
        match disposition.state {
            Some(DeliveryState::TransactionalState(_)) => self.on_incoming_txn_disposition(channel, disposition).await,
            Some(_)
            | None => self.session.on_incoming_disposition(channel, disposition).await,
        }
    }

    async fn on_incoming_detach(
        &mut self,
        channel: IncomingChannel,
        detach: Detach,
    ) -> Result<(), Self::Error> {
        self.session.on_incoming_detach(channel, detach).await
    }

    async fn on_incoming_end(
        &mut self,
        channel: IncomingChannel,
        end: End,
    ) -> Result<(), Self::Error> {
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
        // TODO:
        self.session.on_outgoing_flow(flow)
    }

    fn on_outgoing_transfer(
        &mut self,
        input_handle: InputHandle,
        transfer: Transfer,
        payload: Payload,
    ) -> Result<SessionFrame, Self::Error> {
        // TODO:

        self.session.on_outgoing_transfer(input_handle, transfer, payload)
    }

    fn on_outgoing_disposition(
        &mut self,
        disposition: Disposition,
    ) -> Result<SessionFrame, Self::Error> {
        // TODO:

        self.session.on_outgoing_disposition(disposition)
    }

    fn on_outgoing_detach(&mut self, detach: Detach) -> Result<SessionFrame, Self::Error> {
        self.on_outgoing_detach(detach)
    }
}