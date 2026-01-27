use tokio::sync::mpsc::channel;

use crate::{action::ActionBehavior, executor::Executor, server::Server};

pub struct Builder;

impl Builder {
    pub fn build<A>(action: A) -> (Server<A>, Executor)
    where
        A: ActionBehavior,
    {
        let (task_sender, task_receiver) = channel(16);
        (
            Server::new(action, task_sender),
            Executor::new(task_receiver),
        )
    }
}
