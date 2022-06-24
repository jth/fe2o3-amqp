use fe2o3_amqp_types::messaging::TargetArchetype;

use crate::{session::AllocLinkError, link::{receiver::ReceiverInner, ReceiverLink, target_archetype::{VerifyTargetArchetype, TargetArchetypeExt}}};


#[derive(Debug, thiserror::Error)]
pub(crate) enum LinkAcceptorError<T> 
where
    T: Into<TargetArchetype>
        + TryFrom<TargetArchetype>
        + TargetArchetypeExt
        + Clone
        + Send,
{
    /// Session is in an illegal state
    #[error("Illegal session state")]
    IllegalSessionState,

    // /// Session's max number of handle has reached
    // #[error("Handle max reached")]
    // HandleMaxReached,

    /// Link name is duplicated
    #[error("Link name must be unique")]
    DuplicatedLinkName,

    /// Initial delivery count field MUST NOT be null if role is sender, and it is ignored if the role is receiver.
    /// #[error("Initial delivery count MUST NOT be null if role is sender,")]
    /// InitialDeliveryCountIsNull,
    /// Source field in Attach is Null
    #[error("Source is None")]
    IncomingSourceIsNone(ReceiverInner<ReceiverLink<T>>),

    /// Target field in Attach is Null
    #[error("Target is None")]
    IncomingTargetIsNone(ReceiverInner<ReceiverLink<T>>),

    // /// Reject an incoming attach
    // #[error("Reject attach")]
    // RejectAttach,

    /// Coordinator is not implemented
    #[error("Coordinator is not implemented")]
    CoordinatorNotImplemented(ReceiverInner<ReceiverLink<T>>),
}

impl<T> From<AllocLinkError> for LinkAcceptorError<T> 
where
    T: Into<TargetArchetype>
        + TryFrom<TargetArchetype>
        + TargetArchetypeExt
        + Clone
        + Send,
{
    fn from(value: AllocLinkError) -> Self {
        match value {
            AllocLinkError::IllegalState => Self::IllegalSessionState,
            AllocLinkError::DuplicatedLinkName => Self::DuplicatedLinkName,
        }
    }
}