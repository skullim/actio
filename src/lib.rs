use std::pin::Pin;
use thiserror::Error as ThisError;

pub mod execution;
pub mod factory;
pub mod server;
pub mod submitting;
pub mod task_handle;

type TaskBox = Box<dyn Future<Output = ()> + Send + 'static>;
pub(crate) type PinnedTask = Pin<TaskBox>;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("executor task queue is full")]
    FullExecutor,
    #[error("failed to send cancel request")]
    CancelSendFailure,
}

pub type Result<T> = std::result::Result<T, Error>;
