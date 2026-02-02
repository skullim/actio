use std::pin::Pin;

pub mod execution;
pub mod factory;
pub mod server;
pub mod submitting;
pub mod task_handle;

type TaskBox = Box<dyn Future<Output = ()> + Send + 'static>;
pub(crate) type PinnedTask = Pin<TaskBox>;
