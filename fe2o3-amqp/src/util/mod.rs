//! Common utilities

use fe2o3_amqp_types::messaging::{Target, TargetArchetype};
use fe2o3_amqp_types::transaction::Coordinator;
use futures_util::Future;
use std::ops::Deref;
use std::{pin::Pin, task::Poll, time::Duration};
use tokio::time::Instant;

use tokio::time::Sleep;

mod consumer;
mod producer;
pub use consumer::*;
pub use producer::*;

#[derive(Debug)]
pub(crate) enum Running {
    Continue,
    Stop,
}

#[derive(Debug)]
pub(crate) struct IdleTimeout {
    delay: Pin<Box<Sleep>>,
    duration: Duration,
}

impl IdleTimeout {
    pub fn new(duration: Duration) -> Self {
        let delay = Box::pin(tokio::time::sleep(duration));
        Self { delay, duration }
    }

    pub fn reset(&mut self) {
        let now = Instant::now();
        let next = now + self.duration;
        self.delay.as_mut().reset(next);
    }
}

impl Future for IdleTimeout {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        self.delay.as_mut().poll(cx)
        // .map(|_| IdleTimeoutElapsed {  } )
    }
}

/// An custom type to make a field immutable to
/// prevent accidental mutations
#[derive(Debug)]
pub(crate) struct Constant<T> {
    value: T,
}

impl<T> Constant<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }

    pub fn value(&self) -> &T {
        &self.value
    }
}

impl<T> Deref for Constant<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

/// Shared type state for builders
#[derive(Debug)]
pub struct Uninitialized {}

/// Shared type state for builders
#[derive(Debug)]
pub struct Initialized {}

/// Convenience functions to test the variant of a TargetArchetype
pub trait TargetArchetypeVariant {
    fn is_target(&self) -> bool;

    fn is_coordinator(&self) -> bool;
}

impl TargetArchetypeVariant for Target {
    fn is_target(&self) -> bool {
        true
    }

    fn is_coordinator(&self) -> bool {
        false
    }
}

impl TargetArchetypeVariant for Coordinator {
    fn is_target(&self) -> bool {
        false
    }

    fn is_coordinator(&self) -> bool {
        true
    }
}

impl TargetArchetypeVariant for TargetArchetype {
    fn is_target(&self) -> bool {
        match self {
            TargetArchetype::Target(_) => true,
            TargetArchetype::Coordinator(_) => false,
        }
    }

    fn is_coordinator(&self) -> bool {
        match self {
            TargetArchetype::Target(_) => false,
            TargetArchetype::Coordinator(_) => true,
        }
    }
}
