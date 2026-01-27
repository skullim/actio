use std::pin::Pin;

pub mod action;
pub mod builder;
pub mod executor;
pub mod server;
pub mod task;

type TaskBox = Box<dyn Future<Output = ()>>;
pub(crate) type PinnedTask = Pin<TaskBox>;
