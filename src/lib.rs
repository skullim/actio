use std::pin::Pin;

pub mod builder;
pub mod execution;
pub mod scheduling;
pub mod server;
pub mod task;

type TaskBox = Box<dyn Future<Output = ()>>;
pub(crate) type PinnedTask = Pin<TaskBox>;
