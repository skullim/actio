use std::pin::Pin;

pub mod builder;
pub mod execution;
pub mod scheduling;
pub mod server;

type TaskBox = Box<dyn Future<Output = ()> + Send + 'static>;
pub(crate) type PinnedTask = Pin<TaskBox>;
