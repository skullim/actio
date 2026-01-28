use crate::task::{SimpleTask, TaskHandleWithFeedback, TaskWithFeedback};
use crate::{PinnedTask, server::ServerConcept, task::TaskHandle};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

pub enum Error {
    Full,
    RejectedByExecutor,
}

pub trait ScheduleTask<G> {
    type TaskHandle;

    fn schedule(&mut self, goal: G) -> Result<Self::TaskHandle, Error>;
}

pub struct SimpleTaskScheduler<S> {
    server: S,
    task_sender: Sender<PinnedTask>,
}

impl<S> SimpleTaskScheduler<S> {
    pub fn new(server: S, task_sender: Sender<PinnedTask>) -> Self {
        Self {
            server,
            task_sender,
        }
    }
}

impl<G, R, A, F> ScheduleTask<G> for SimpleTaskScheduler<A>
where
    A: ServerConcept<G, Task = SimpleTask<F>>,
    F: Future<Output = R> + 'static,
    R: 'static,
{
    type TaskHandle = TaskHandle<R>;

    fn schedule(&mut self, goal: G) -> Result<Self::TaskHandle, Error> {
        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
        let (result_tx, result_rx) = oneshot::channel::<R>();

        let SimpleTask { task } = self.server.create(goal);
        let server_task = async move {
            tokio::select! {
                _ = cancel_rx => {}
                r = task => { let _ = result_tx.send(r); }
            }
        };

        self.task_sender
            .try_send(Box::pin(server_task))
            .map_err(|_| Error::RejectedByExecutor)?;

        Ok(TaskHandle::new(cancel_tx, result_rx))
    }
}

pub struct TaskWithFeedbackScheduler<S> {
    server: S,
    task_sender: Sender<PinnedTask>,
}

impl<S> TaskWithFeedbackScheduler<S> {
    pub fn new(server: S, task_sender: Sender<PinnedTask>) -> Self {
        Self {
            server,
            task_sender,
        }
    }
}

impl<G, R, S, F, Fb> ScheduleTask<G> for TaskWithFeedbackScheduler<S>
where
    S: ServerConcept<G, Task = TaskWithFeedback<F, Fb>>,
    F: Future<Output = R> + 'static,
    R: 'static,
    Fb: 'static,
{
    type TaskHandle = TaskHandleWithFeedback<R, Fb>;

    fn schedule(&mut self, goal: G) -> Result<Self::TaskHandle, Error> {
        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
        let (result_tx, result_rx) = oneshot::channel::<R>();

        let TaskWithFeedback { task, feedback_rx } = self.server.create(goal);
        let server_task = async move {
            tokio::select! {
                _ = cancel_rx => {}
                r = task => { let _ = result_tx.send(r); }
            }
        };

        self.task_sender
            .try_send(Box::pin(server_task))
            .map_err(|_| Error::RejectedByExecutor)?;

        Ok(TaskHandleWithFeedback::new(
            cancel_tx,
            result_rx,
            feedback_rx,
        ))
    }
}
