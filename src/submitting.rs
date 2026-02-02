// Capabilities that change the server async task
// 1. Sending result (req)
// 2. cancelling future (opt)
// 3. sending task execution ctx on cancel (opt)

// Data needed when creating server task
// 1. Task (req)
// 2. Feedback (receiver) (opt)
// 3. Task execution ctx (receiver) (opt)
// Both affect how Task Handle is built

// Different Task Handles that contain 0 or 1 of followings:
// 1. result receiver (req)
// 2. cancel sender (opt)
// 3. feedback receiver (opt)
// 4. act as wrapper to await result, but let server visit result (supports stateful server) (opt)

use std::marker::PhantomData;

use crate::server::{Outcome, ServerOutcome, TaskStateSnapshotReceiver};
use crate::task_handle::{
    CancelConfigMarker, NoCancel, StatefulTaskHandle, TaskHandle, WithCancel,
};
use crate::{PinnedTask, server::ServerConcept};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum Error {
    Full,
    RejectedByExecutor,
}

pub trait SubmitGoal<G> {
    type TaskHandle;
    type Server: ServerConcept;

    fn submit(&mut self, server: &mut Self::Server, goal: G) -> Result<Self::TaskHandle, Error>;
}

pub(crate) struct GoalSubmitter<S, CF> {
    task_sender: Sender<PinnedTask>,
    phantom: PhantomData<(S, CF)>,
}

impl<S, CF> GoalSubmitter<S, CF> {
    pub fn new(task_sender: Sender<PinnedTask>) -> Self {
        Self {
            task_sender,
            phantom: PhantomData,
        }
    }
}

impl<S, CF> SubmitGoal<S::Goal> for GoalSubmitter<S, CF>
where
    S: ServerConcept,
    CF: CancelChannelFactory,
{
    type Server = S;
    type TaskHandle = TaskHandle<S, CF>;

    fn submit(&mut self, server: &mut S, goal: S::Goal) -> Result<Self::TaskHandle, Error> {
        let (result_sender, result_receiver) = ResultChannelFactory::channel::<ServerOutcome<S>>();
        let (cancel_sender, cancel_receiver) = CF::channel();

        let task_with_ctx = server.create(goal);
        let task = task_with_ctx.task;
        let feedback_receiver = task_with_ctx.feedback_receiver;
        let mut task_state_snapshot_receiver = task_with_ctx.task_state_snapshot_receiver;

        let task = async move {
            let outcome = tokio::select! {
                _ = cancel_receiver.recv() => {
                    let snapshot = task_state_snapshot_receiver.recv();
                    Outcome::Cancelled(snapshot)
                }
                r = task => r,
            };
            let _ = result_sender.send(outcome);
        };

        self.task_sender
            .try_send(Box::pin(task))
            .map_err(|_| Error::RejectedByExecutor)?;

        Ok(Self::TaskHandle::new(
            result_receiver,
            cancel_sender,
            feedback_receiver,
        ))
    }
}

pub(crate) struct StatefulGoalSubmitter<S, CF> {
    submitter: GoalSubmitter<S, CF>,
}

impl<S, CF> StatefulGoalSubmitter<S, CF> {
    pub(crate) fn new(submitter: GoalSubmitter<S, CF>) -> Self {
        Self { submitter }
    }
}

impl<S, CF> SubmitGoal<S::Goal> for StatefulGoalSubmitter<S, CF>
where
    S: ServerConcept,
    CF: CancelChannelFactory,
{
    type Server = S;
    type TaskHandle = StatefulTaskHandle<S, CF>;

    fn submit(&mut self, server: &mut S, goal: S::Goal) -> Result<Self::TaskHandle, Error> {
        let handle = self.submitter.submit(server, goal)?;
        Ok(StatefulTaskHandle::new(handle))
    }
}

struct ResultChannelFactory;

impl ResultChannelFactory {
    fn channel<O>() -> (oneshot::Sender<O>, oneshot::Receiver<O>) {
        tokio::sync::oneshot::channel()
    }
}

pub trait CancelChannelFactory {
    type Sender: CancelConfigMarker;
    fn channel() -> (Self::Sender, impl CancelReceiver);
}

pub trait CancelReceiver: Send + 'static {
    fn recv(self) -> impl Future<Output = ()> + Send + 'static;
}

impl CancelReceiver for oneshot::Receiver<()> {
    async fn recv(self) {
        self.await.unwrap()
    }
}

pub struct NoCancelReceiver;

impl CancelReceiver for NoCancelReceiver {
    async fn recv(self) {
        std::future::pending::<()>().await
    }
}

pub struct NoCancelChannel;

impl CancelChannelFactory for NoCancelChannel {
    type Sender = NoCancel;
    fn channel() -> (Self::Sender, impl CancelReceiver) {
        (NoCancel, NoCancelReceiver)
    }
}

pub struct CancelChannel;

impl CancelChannelFactory for CancelChannel {
    type Sender = WithCancel<oneshot::Sender<()>>;
    fn channel() -> (Self::Sender, impl CancelReceiver) {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        (WithCancel::new(sender), receiver)
    }
}
