use std::pin::Pin;

use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

use crate::{PinnedTask, action::ActionBehavior, task::TaskHandle};

pub enum Error {
    Full,
}

pub struct Server<A> {
    action: A,
    task_sender: Sender<PinnedTask>,
}

impl<A> Server<A>
where
    A: ActionBehavior,
{
    pub fn new(action: A, task_sender: Sender<PinnedTask>) -> Self {
        Self {
            action,
            task_sender,
        }
    }

    pub fn send_task(&mut self, goal: A::Goal) -> Result<TaskHandle<A::Result>, Error> {
        let (cancel_sender, cancel_receiver) = oneshot::channel::<()>();
        let (result_sender, result_receiver) = oneshot::channel();
        let action_task = self.action.create(goal);
        let server_task = async move {
            tokio::select! {
                _  = cancel_receiver => {

                },
                result = action_task => {
                    //@todo visit the result but needs to wrap action in Arc.
                    result_sender.send(result);
                }
            }
        };
        self.task_sender.try_send(Pin::from(Box::new(server_task)));
        Ok(TaskHandle::new(cancel_sender, result_receiver))
    }
}
