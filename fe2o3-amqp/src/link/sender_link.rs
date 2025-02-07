use fe2o3_amqp_types::definitions::SequenceNo;
use futures_util::Future;

use super::{resumption::resume_delivery, *};

#[async_trait]
impl<T> endpoint::SenderLink for SenderLink<T>
where
    T: Into<TargetArchetype>
        + TryFrom<TargetArchetype>
        + VerifyTargetArchetype
        + Clone
        + Send
        + Sync,
{
    type FlowError = FlowError;
    type TransferError = LinkStateError;
    type DispositionError = DispositionError;

    /// Set and send flow state
    async fn send_flow(
        &mut self,
        writer: &mpsc::Sender<LinkFrame>,
        delivery_count: Option<SequenceNo>,
        available: Option<u32>,
        echo: bool,
    ) -> Result<(), Self::FlowError> {
        let handle = self
            .output_handle
            .clone()
            .ok_or(Self::FlowError::IllegalState)?
            .into();

        let flow = match (delivery_count, available) {
            (Some(delivery_count), Some(available)) => {
                let mut writer = self.flow_state.as_ref().lock.write().await;
                writer.delivery_count = delivery_count;
                writer.available = available;
                LinkFlow {
                    handle,
                    delivery_count: Some(delivery_count),
                    // TODO: "last known value"???
                    // The sender endpoint sets this to the last known value seen from the receiver.
                    link_credit: Some(writer.link_credit),
                    available: Some(available),
                    // When flow state is sent from the sender to the receiver, this field
                    // contains the actual drain mode of the sender
                    drain: writer.drain,
                    echo,
                    properties: writer.properties.clone(),
                }
            }
            (Some(delivery_count), None) => {
                let mut writer = self.flow_state.as_ref().lock.write().await;
                writer.delivery_count = delivery_count;
                LinkFlow {
                    handle,
                    delivery_count: Some(delivery_count),
                    // TODO: "last known value"???
                    // The sender endpoint sets this to the last known value seen from the receiver.
                    link_credit: Some(writer.link_credit),
                    available: Some(writer.available),
                    // When flow state is sent from the sender to the receiver, this field
                    // contains the actual drain mode of the sender
                    drain: writer.drain,
                    echo,
                    properties: writer.properties.clone(),
                }
            }
            (None, Some(available)) => {
                let mut writer = self.flow_state.as_ref().lock.write().await;
                writer.available = available;
                LinkFlow {
                    handle,
                    delivery_count: Some(writer.delivery_count),
                    // TODO: "last known value"???
                    // The sender endpoint sets this to the last known value seen from the receiver.
                    link_credit: Some(writer.link_credit),
                    available: Some(available),
                    // When flow state is sent from the sender to the receiver, this field
                    // contains the actual drain mode of the sender
                    drain: writer.drain,
                    echo,
                    properties: writer.properties.clone(),
                }
            }
            (None, None) => {
                let reader = self.flow_state.as_ref().lock.read().await;
                LinkFlow {
                    handle,
                    delivery_count: Some(reader.delivery_count),
                    // TODO: "last known value"???
                    // The sender endpoint sets this to the last known value seen from the receiver.
                    link_credit: Some(reader.link_credit),
                    available: Some(reader.available),
                    // When flow state is sent from the sender to the receiver, this field
                    // contains the actual drain mode of the sender
                    drain: reader.drain,
                    echo,
                    properties: reader.properties.clone(),
                }
            }
        };
        writer
            .send(LinkFrame::Flow(flow))
            .await
            .map_err(|_| Self::FlowError::IllegalSessionState)
    }

    async fn send_payload<Fut>(
        &mut self,
        writer: &mpsc::Sender<LinkFrame>,
        detached: Fut,
        payload: Payload,
        message_format: MessageFormat,
        settled: Option<bool>,
        state: Option<DeliveryState>,
        batchable: bool,
    ) -> Result<Settlement, Self::TransferError>
    where
        Fut: Future<Output = Option<LinkFrame>> + Send,
    {
        use crate::endpoint::LinkDetach;
        use crate::util::Consume;

        let tag = tokio::select! {
            tag = self.flow_state.consume(1) => {
                // link-credit is defined as
                // "The current maximum number of messages that can be handled
                // at the receiver endpoint of the link"

                // Draining should already set the link credit to 0, causing
                // sender to wait for new link credit
                tag
            },
            frame = detached => {
                match frame {
                    // If remote has detached the link
                    Some(LinkFrame::Detach(detach)) => {
                        // FIXME: if the sender is not trying to send anything, this is
                        // probably not responsive enough
                        let closed = detach.closed;
                        self.send_detach(writer, closed, None).await?;
                        let result = self.on_incoming_detach(detach).await;

                        match (result, closed) {
                            (Ok(_), true) => return Err(Self::TransferError::RemoteClosed),
                            (Ok(_), false) => return Err(Self::TransferError::RemoteDetached),
                            (Err(err), _) => return Err(Self::TransferError::from(err)),
                        }
                    },
                    _ => {
                        // Other frames should not forwarded to the sender by the session
                        return Err(Self::TransferError::ExpectImmediateDetach)
                    }
                }
            }
        };

        let handle = self
            .output_handle
            .clone()
            .ok_or(Self::TransferError::IllegalState)?
            .into();

        // Delivery count is incremented when consuming credit
        let delivery_tag = DeliveryTag::from(tag);

        let settled = match self.snd_settle_mode {
            SenderSettleMode::Settled => true,
            SenderSettleMode::Unsettled => false,
            // If not set on the first (or only) transfer for a (multi-transfer)
            // delivery, then the settled flag MUST be interpreted as being false.
            SenderSettleMode::Mixed => settled.unwrap_or(false),
        };

        // If true, the resume flag indicates that the transfer is being used to reassociate an
        // unsettled delivery from a dissociated link endpoint
        let resume = false;

        let transfer = Transfer {
            handle,
            delivery_id: None, // This will be set by the session
            delivery_tag: Some(delivery_tag.clone()),
            message_format: Some(message_format),
            settled: Some(settled),
            more: false, // This will be changed later

            // If not set, this value is defaulted to the value negotiated
            // on link attach.
            rcv_settle_mode: None,
            state,
            resume,
            aborted: false,
            batchable,
        };

        self.send_payload_with_transfer(writer, transfer, payload)
            .await
    }

    async fn send_payload_with_transfer(
        &mut self,
        writer: &mpsc::Sender<LinkFrame>,
        mut transfer: Transfer,
        mut payload: Payload,
    ) -> Result<Settlement, Self::TransferError> {
        let settled = transfer.settled.unwrap_or(match self.snd_settle_mode {
            SenderSettleMode::Settled => true,
            SenderSettleMode::Unsettled => false,
            SenderSettleMode::Mixed => false,
        });
        let delivery_tag = transfer
            .delivery_tag
            .clone()
            .ok_or(Self::TransferError::IllegalState)?;
        let input_handle = self
            .input_handle
            .clone()
            .ok_or(Self::TransferError::IllegalState)?;

        // Keep a copy for unsettled message
        // Clone should be very cheap on Bytes
        let payload_copy = payload.clone();

        // Check message size
        // If this field is zero or unset, there is no maximum size imposed by the link endpoint.
        let more = (self.max_message_size != 0) && (payload.len() as u64 > self.max_message_size);
        if !more {
            // TODO: Clone should be very cheap on Bytes
            transfer.more = false;
            send_transfer(writer, input_handle, transfer, payload.clone()).await?;
        } else {
            // Send the first frame
            let partial = payload.split_to(self.max_message_size as usize);
            transfer.more = true;
            send_transfer(writer, input_handle.clone(), transfer.clone(), partial).await?;

            // Send the transfers in the middle
            while payload.len() > self.max_message_size as usize {
                let partial = payload.split_to(self.max_message_size as usize);
                transfer.delivery_tag = None;
                transfer.message_format = None;
                transfer.settled = None;
                send_transfer(writer, input_handle.clone(), transfer.clone(), partial).await?;
            }

            // Send the last transfer
            // For messages that are too large to fit within the maximum frame size, additional
            // data MAY be trans- ferred in additional transfer frames by setting the more flag on
            // all but the last transfer frame
            transfer.more = false;
            send_transfer(writer, input_handle, transfer, payload).await?;
        }

        match settled {
            true => Ok(Settlement::Settled(delivery_tag)),
            // If not set on the first (or only) transfer for a (multi-transfer)
            // delivery, then the settled flag MUST be interpreted as being false.
            false => {
                let (tx, rx) = oneshot::channel();
                let unsettled = UnsettledMessage::new(payload_copy, tx);
                {
                    let mut guard = self.unsettled.write().await;
                    guard
                        .get_or_insert(BTreeMap::new())
                        .insert(delivery_tag.clone(), unsettled);
                }

                Ok(Settlement::Unsettled {
                    delivery_tag,
                    outcome: rx,
                })
            }
        }
    }

    async fn dispose(
        &mut self,
        writer: &mpsc::Sender<LinkFrame>,
        delivery_id: DeliveryNumber,
        delivery_tag: DeliveryTag,
        settled: bool,
        state: DeliveryState,
        batchable: bool,
    ) -> Result<(), Self::DispositionError> {
        if let SenderSettleMode::Settled = self.snd_settle_mode {
            return Ok(());
        }

        {
            let mut lock = self.unsettled.write().await;
            if settled {
                if let Some(msg) = lock.as_mut().and_then(|m| m.remove(&delivery_tag)) {
                    let _ = msg.settle();
                }
            } else if let Some(msg) = lock.as_mut().and_then(|m| m.get_mut(&delivery_tag)) {
                *msg.state_mut() = Some(state.clone());
            }
        }

        send_disposition(writer, delivery_id, None, settled, Some(state), batchable).await
    }

    async fn batch_dispose(
        &mut self,
        writer: &mpsc::Sender<LinkFrame>,
        mut ids_and_tags: Vec<(DeliveryNumber, DeliveryTag)>,
        settled: bool,
        state: DeliveryState,
        batchable: bool,
    ) -> Result<(), Self::DispositionError> {
        if let SenderSettleMode::Settled = self.snd_settle_mode {
            return Ok(());
        }

        let mut first = None;
        let mut last = None;

        // TODO: Is sort necessary?
        ids_and_tags.sort_by(|left, right| left.0.cmp(&right.0));

        let mut lock = self.unsettled.write().await;

        // Find continuous ranges
        for (delivery_id, delivery_tag) in ids_and_tags {
            if settled {
                if let Some(msg) = lock.as_mut().and_then(|m| m.remove(&delivery_tag)) {
                    let _ = msg.settle();
                }
            } else if let Some(msg) = lock.as_mut().and_then(|m| m.get_mut(&delivery_tag)) {
                *msg.state_mut() = Some(state.clone());
            }

            match (first, last) {
                // First pair
                (None, _) => first = Some(delivery_id),
                // Second pair
                (Some(first_id), None) => {
                    // Find discontinuity
                    if delivery_id - first_id > 1 {
                        send_disposition(
                            writer,
                            first_id,
                            None,
                            settled,
                            Some(state.clone()),
                            batchable,
                        )
                        .await?;
                    }
                    last = Some(delivery_id);
                }
                // Third and more
                (Some(first_id), Some(last_id)) => {
                    // Find discontinuity
                    if delivery_id - last_id > 1 {
                        send_disposition(
                            writer,
                            first_id,
                            Some(last_id),
                            settled,
                            Some(state.clone()),
                            batchable,
                        )
                        .await?;
                    }
                    last = Some(delivery_id);
                }
            }
        }

        // if there is only one message to dispose
        if let (Some(first_id), None) = (first, last) {
            send_disposition(writer, first_id, None, settled, Some(state), batchable).await?;
        }
        Ok(())
    }
}

#[inline]
async fn send_transfer(
    writer: &mpsc::Sender<LinkFrame>,
    input_handle: InputHandle,
    transfer: Transfer,
    payload: Payload,
) -> Result<(), LinkStateError> {
    let frame = LinkFrame::Transfer {
        input_handle,
        performative: transfer,
        payload,
    };
    writer
        .send(frame)
        .await
        .map_err(|_| LinkStateError::IllegalSessionState)
}

#[inline]
async fn send_disposition(
    writer: &mpsc::Sender<LinkFrame>,
    first: DeliveryNumber,
    last: Option<DeliveryNumber>,
    settled: bool,
    state: Option<DeliveryState>,
    batchable: bool,
) -> Result<(), IllegalLinkStateError> {
    let disposition = Disposition {
        role: Role::Sender,
        first,
        last,
        settled,
        state,
        batchable,
    };
    let frame = LinkFrame::Disposition(disposition);
    writer
        .send(frame)
        .await
        .map_err(|_| IllegalLinkStateError::IllegalSessionState)
}

impl<T> SenderLink<T> {
    async fn handle_unsettled_in_attach(
        &mut self,
        remote_unsettled: Option<BTreeMap<DeliveryTag, Option<DeliveryState>>>,
    ) -> Result<SenderAttachExchange, SenderAttachError> {
        let mut guard = self.unsettled.write().await;
        let v: Vec<(DeliveryTag, ResumingDelivery)> = match (guard.take(), remote_unsettled) {
            (None, None) => return Ok(SenderAttachExchange::Complete),
            (None, Some(remote_map)) => {
                if remote_map.is_empty() {
                    return Ok(SenderAttachExchange::Complete);
                }

                remote_map
                    .into_keys()
                    .map(|delivery_tag| (delivery_tag, ResumingDelivery::Abort))
                    .collect()
            }
            (Some(local_map), None) => {
                if local_map.is_empty() {
                    return Ok(SenderAttachExchange::Complete);
                }

                local_map
                    .into_iter()
                    .filter_map(|(tag, local)| {
                        resume_delivery(local, None).map(|resume| (tag, resume))
                    })
                    .collect()
            }
            (Some(local_map), Some(mut remote_map)) => {
                if local_map.is_empty() && remote_map.is_empty() {
                    return Ok(SenderAttachExchange::Complete);
                }

                let local: Vec<(DeliveryTag, ResumingDelivery)> = local_map
                    .into_iter()
                    .filter_map(|(tag, local)| {
                        let remote = remote_map.remove(&tag);
                        resume_delivery(local, remote).map(|resume| (tag, resume))
                    })
                    .collect();
                let remote = remote_map
                    .into_keys()
                    .map(|tag| (tag, ResumingDelivery::Abort));
                local.into_iter().chain(remote).collect()
            }
        };

        match self.local_state {
            LinkState::IncompleteAttachReceived
            | LinkState::IncompleteAttachSent
            | LinkState::IncompleteAttachExchanged => {
                Ok(SenderAttachExchange::IncompleteUnsettled(v))
            }
            _ => Ok(SenderAttachExchange::Resume(v)),
        }
    }
}

#[async_trait]
impl<T> endpoint::LinkAttach for SenderLink<T>
where
    T: Into<TargetArchetype>
        + TryFrom<TargetArchetype>
        + VerifyTargetArchetype
        + Clone
        + Send
        + Sync,
{
    type AttachExchange = SenderAttachExchange;
    type AttachError = SenderAttachError;

    async fn on_incoming_attach(
        &mut self,
        remote_attach: Attach,
    ) -> Result<Self::AttachExchange, Self::AttachError> {
        use self::source::VerifySource;

        match (&self.local_state, remote_attach.incomplete_unsettled) {
            (LinkState::AttachSent, false) => {
                self.local_state = LinkState::Attached;
            }
            (LinkState::IncompleteAttachSent, false) => {
                self.local_state = LinkState::IncompleteAttachExchanged;
            }
            (LinkState::Unattached, false) | (LinkState::Detached, false) => {
                self.local_state = LinkState::AttachReceived; // re-attaching
            }
            (LinkState::AttachSent, true) | (LinkState::IncompleteAttachSent, true) => {
                self.local_state = LinkState::IncompleteAttachExchanged;
            }
            (LinkState::Unattached, true) | (LinkState::Detached, true) => {
                self.local_state = LinkState::IncompleteAttachReceived; // re-attaching
            }
            _ => return Err(SenderAttachError::IllegalState),
        };

        self.input_handle = Some(InputHandle::from(remote_attach.handle));

        // In this case, the sender is considered to hold the authoritative version of the
        // version of the source properties
        // TODO: is verification necessary?
        match (&self.source, &remote_attach.source) {
            (Some(local_source), Some(remote_source)) => {
                local_source.verify_as_sender(remote_source)?;
            }
            (_, None) => return Err(SenderAttachError::IncomingSourceIsNone),
            _ => {}
        }

        let target = remote_attach
            .target
            .map(|t| T::try_from(*t))
            .transpose()
            .map_err(|_| SenderAttachError::CoordinatorIsNotImplemented)?;

        // Note that it is the responsibility of the transaction controller to
        // verify that the capabilities of the controller meet its requirements.
        //
        // the receiver is considered to hold the authoritative version of the target properties
        match (&self.target, &target) {
            (Some(local_target), Some(remote_target)) => {
                local_target.verify_as_sender(remote_target)?
            }
            (_, None) => return Err(SenderAttachError::IncomingTargetIsNone),
            _ => {}
        }
        self.target = target;

        // The sender SHOULD respect the receiver’s desired settlement mode if the receiver
        // initiates the attach exchange and the sender supports the desired mode
        if self.rcv_settle_mode != remote_attach.rcv_settle_mode {
            return Err(SenderAttachError::RcvSettleModeNotSupported);
        }

        if self.snd_settle_mode != remote_attach.snd_settle_mode {
            return Err(SenderAttachError::SndSettleModeNotSupported);
        }

        self.max_message_size =
            get_max_message_size(self.max_message_size, remote_attach.max_message_size);

        self.handle_unsettled_in_attach(remote_attach.unsettled)
            .await
    }

    async fn send_attach(
        &mut self,
        writer: &mpsc::Sender<LinkFrame>,
        session: &mpsc::Sender<SessionControl>,
        is_reattaching: bool,
    ) -> Result<(), Self::AttachError> {
        self.send_attach_inner(writer, session, is_reattaching)
            .await?;
        Ok(())
    }
}

impl<T> endpoint::Link for SenderLink<T>
where
    T: Into<TargetArchetype>
        + TryFrom<TargetArchetype>
        + VerifyTargetArchetype
        + Clone
        + Send
        + Sync,
{
    fn role() -> Role {
        Role::Sender
    }
}

#[async_trait]
impl<T> endpoint::LinkExt for SenderLink<T>
where
    T: Into<TargetArchetype>
        + TryFrom<TargetArchetype>
        + VerifyTargetArchetype
        + Clone
        + Send
        + Sync,
{
    type FlowState = SenderFlowState;
    type Unsettled = ArcSenderUnsettledMap;
    type Target = T;

    fn local_state(&self) -> &LinkState {
        &self.local_state
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn output_handle(&self) -> &Option<OutputHandle> {
        &self.output_handle
    }

    fn output_handle_mut(&mut self) -> &mut Option<OutputHandle> {
        &mut self.output_handle
    }

    fn flow_state(&self) -> &Self::FlowState {
        &self.flow_state
    }

    fn unsettled(&self) -> &Self::Unsettled {
        &self.unsettled
    }

    fn rcv_settle_mode(&self) -> &ReceiverSettleMode {
        &self.rcv_settle_mode
    }

    fn target(&self) -> &Option<Self::Target> {
        &self.target
    }

    async fn exchange_attach(
        &mut self,
        writer: &mpsc::Sender<LinkFrame>,
        reader: &mut mpsc::Receiver<LinkFrame>,
        session: &mpsc::Sender<SessionControl>,
        is_reattaching: bool,
    ) -> Result<Self::AttachExchange, SenderAttachError> {
        // Send out local attach
        self.send_attach(writer, session, is_reattaching).await?;

        // Wait for remote attach
        let remote_attach = match reader
            .recv()
            .await
            .ok_or(SenderAttachError::IllegalSessionState)?
        {
            LinkFrame::Attach(attach) => attach,
            _ => return Err(SenderAttachError::NonAttachFrameReceived),
        };

        self.on_incoming_attach(remote_attach).await
    }

    #[instrument(skip_all)]
    async fn handle_attach_error(
        &mut self,
        attach_error: SenderAttachError,
        writer: &mpsc::Sender<LinkFrame>,
        reader: &mut mpsc::Receiver<LinkFrame>,
        session: &mpsc::Sender<SessionControl>,
    ) -> SenderAttachError {
        match attach_error {
            SenderAttachError::IllegalSessionState
            | SenderAttachError::IllegalState
            | SenderAttachError::NonAttachFrameReceived
            | SenderAttachError::ExpectImmediateDetach
            | SenderAttachError::RemoteClosedWithError(_) => attach_error,

            SenderAttachError::DuplicatedLinkName => {
                let error = definitions::Error::new(
                    SessionError::HandleInUse,
                    "Link name is in use".to_string(),
                    None,
                );
                session
                    .send(SessionControl::End(Some(error)))
                    .await
                    .map(|_| attach_error)
                    .unwrap_or(SenderAttachError::IllegalSessionState)
            }

            SenderAttachError::SndSettleModeNotSupported
            | SenderAttachError::RcvSettleModeNotSupported
            | SenderAttachError::IncomingSourceIsNone
            | SenderAttachError::IncomingTargetIsNone => {
                // Just send detach immediately
                let err = self
                    .send_detach(writer, true, None)
                    .await
                    .map(|_| attach_error)
                    .unwrap_or(SenderAttachError::IllegalSessionState);
                recv_detach(self, reader, err).await
            }

            SenderAttachError::CoordinatorIsNotImplemented
            | SenderAttachError::SourceAddressIsSomeWhenDynamicIsTrue
            | SenderAttachError::TargetAddressIsNoneWhenDynamicIsTrue
            | SenderAttachError::DynamicNodePropertiesIsSomeWhenDynamicIsFalse => {
                try_detach_with_error(self, attach_error, writer, reader).await
            }
            #[cfg(feature = "transaction")]
            SenderAttachError::DesireTxnCapabilitiesNotSupported => {
                try_detach_with_error(self, attach_error, writer, reader).await
            }
        }
    }
}

async fn try_detach_with_error<L>(
    link: &mut L,
    attach_error: SenderAttachError,
    writer: &mpsc::Sender<LinkFrame>,
    reader: &mut mpsc::Receiver<LinkFrame>,
) -> SenderAttachError
where
    L: LinkDetach,
{
    match (&attach_error).try_into() {
        Ok(err) => {
            match link.send_detach(writer, true, Some(err)).await {
                Ok(_) => match reader.recv().await {
                    Some(LinkFrame::Detach(remote_detach)) => {
                        let _ = link.on_incoming_detach(remote_detach).await; // FIXME: hadnle detach errors?
                        attach_error
                    }
                    Some(_) => SenderAttachError::NonAttachFrameReceived,
                    None => SenderAttachError::IllegalSessionState,
                },
                Err(_) => SenderAttachError::IllegalSessionState,
            }
        }
        Err(_) => attach_error,
    }
}

async fn recv_detach<T>(
    link: &mut SenderLink<T>,
    reader: &mut mpsc::Receiver<LinkFrame>,
    err: SenderAttachError,
) -> SenderAttachError
where
    T: Into<TargetArchetype>
        + TryFrom<TargetArchetype>
        + VerifyTargetArchetype
        + Clone
        + Send
        + Sync,
{
    match reader.recv().await {
        Some(LinkFrame::Detach(remote_detach)) => {
            match link.on_incoming_detach(remote_detach).await {
                Ok(_) => err,
                Err(detach_error) => detach_error.try_into().unwrap_or(err),
            }
        }
        Some(_) => SenderAttachError::NonAttachFrameReceived,
        None => SenderAttachError::IllegalSessionState,
    }
}
