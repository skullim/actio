use tokio::sync::mpsc::Receiver;

use futures::StreamExt;
use futures::stream::FuturesUnordered;
use std::future::poll_fn;
use std::task::Poll;

use crate::PinnedTask;

pub struct Executor {
    task_receiver: Receiver<PinnedTask>,
}

impl Executor {
    pub(crate) fn new(task_receiver: Receiver<PinnedTask>) -> Self {
        Self { task_receiver }
    }

    pub async fn execute(&mut self) {
        let mut tasks = FuturesUnordered::new();

        loop {
            let execute_next_task_fut = poll_fn(|cx| {
                if tasks.is_empty() {
                    Poll::Pending
                } else {
                    tasks.poll_next_unpin(cx)
                }
            });

            tokio::select! {
                Some(task) = self.task_receiver.recv() => {
                    tasks.push(task);
                },
                _ = execute_next_task_fut => {
                }

            }
        }
    }
}
