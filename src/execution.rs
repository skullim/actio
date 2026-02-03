use tokio::sync::mpsc::Receiver;

use futures::StreamExt;
use futures::stream::FuturesUnordered;
use std::future::poll_fn;
use std::task::Poll;
use tracing::trace;

use crate::TaskPin;

pub struct Executor {
    tasks: FuturesUnordered<TaskPin>,
    task_receiver: Receiver<TaskPin>,
}

impl Executor {
    pub(crate) fn new(task_receiver: Receiver<TaskPin>) -> Self {
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
                task = self.task_receiver.recv() => {
                    match task {
                        Some(task) => {
                            trace!("pushing new task");
                            self.tasks.push(task);
                        }
                        None => {
                            trace!("task channel closed");
                            break;
                        }
                    }

                },
                _ = execute_next_task_fut => {
                    trace!("finished executing task");
                },
            }
        }
    }
}
