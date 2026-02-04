use futures::{StreamExt, channel::mpsc::Receiver, stream::FuturesUnordered};
use std::future::poll_fn;
use std::task::Poll;
use tokio::select;
use tracing::{trace, warn};

use crate::TaskPin;

/// Drives submitted tasks to completion.
///
/// Receives tasks from an internal queue and polls all in-flight tasks.
/// Note: `FuturesUnordered` completes with `None` when empty, so `execute()` uses
/// a small `poll_fn` shim to treat “empty” as “not ready yet”.
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
    /// Poll submitted tasks
    pub async fn execute(&mut self) {
        loop {
            let next_task_poll_fn = poll_fn(|cx| {
                if self.tasks.is_empty() {
                    Poll::Pending
                } else {
                    self.tasks.poll_next_unpin(cx)
                }
            });

            select! {
                task = self.task_receiver.next() => {
                    if let Some(task) = task {
                            trace!("pushing new task");
                            self.tasks.push(task);
                    }
                    else {
                            warn!("task channel closed, no new tasks can be sent");
                            break;
                    }

                },
                _ = next_task_poll_fn => {
                    trace!("finished executing task");

                },
            }
        }
    }
}
