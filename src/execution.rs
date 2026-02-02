use tokio::sync::mpsc::Receiver;

use futures::StreamExt;
use futures::stream::FuturesUnordered;
use std::future::poll_fn;
use std::task::Poll;
use tracing::trace;

use crate::PinnedTask;

pub struct Executor {
    tasks: FuturesUnordered<PinnedTask>,
    task_receiver: Receiver<PinnedTask>,
}

impl Executor {
    pub(crate) fn new(task_receiver: Receiver<PinnedTask>) -> Self {
        Self {
            task_receiver,
            tasks: FuturesUnordered::new(),
        }
    }

    pub async fn execute(&mut self) {
        loop {
            let execute_next_task_fut = poll_fn(|cx| {
                if self.tasks.is_empty() {
                    Poll::Pending
                } else {
                    self.tasks.poll_next_unpin(cx)
                }
            });

            tokio::select! {
                Some(task) = self.task_receiver.recv() => {
                    trace!("pushing new task");
                    self.tasks.push(task);
                },
                _ = execute_next_task_fut => {
                    trace!("finished executing task");

                }

            }
        }
    }
}
