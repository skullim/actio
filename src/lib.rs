use std::pin::Pin;
use thiserror::Error as ThisError;

mod execution;
mod factory;
mod server;
mod submitting;
mod task_handle;

type TaskBox = Box<dyn Future<Output = ()> + Send + 'static>;
pub(crate) type TaskPin = Pin<TaskBox>;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("executor task queue is full")]
    FullExecutor,
    #[error("failed to send cancel request")]
    CancelSendFailure,
}

pub type Result<T> = std::result::Result<T, Error>;

pub use execution::Executor;
pub use factory::Factory;
pub use server::{
    FeedbackReceiverMarker, NoFeedback, NoTaskStateSnapshot, Outcome, ServerConcept, ServerOutcome,
    ServerSnapshot, ServerTask, VisitOutcome, WithFeedback, WithTaskStateSnapshot,
};
pub use submitting::{CancelChannel, NoCancelChannel, SubmitGoal};
pub use task_handle::{NoCancel, StatefulTaskHandle, TaskHandle, VisitableOutcome, WithCancel};

#[cfg(test)]
pub use server::{MockServerConcept, MockVisitOutcome};
