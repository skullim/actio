use std::marker::PhantomData;

use crate::server::{Outcome, ServerConcept, ServerOutcome, TaskStateSnapshotReceiver};
use crate::task_handle::{
    CancelConfigMarker, NoCancel, StatefulTaskHandle, TaskHandle, WithCancel,
};
use crate::{Error, Result, TaskPin};
use futures::channel::{mpsc::Sender, oneshot};

/// Goal submission interface.
/// Dispatches a `Goal` to a concrete [`ServerConcept`] implementation, enqueues the resulting task
/// for execution, and returns a handle to interact with the task.
pub trait SubmitGoal<G> {
    type TaskHandle;
    type Server: ServerConcept;

    fn submit(&mut self, server: &mut Self::Server, goal: G) -> Result<Self::TaskHandle>;
}

pub struct GoalSubmitter<S, CF> {
    task_sender: Sender<TaskPin>,
    phantom: PhantomData<(S, CF)>,
}

impl<S, CF> GoalSubmitter<S, CF> {
    pub fn new(task_sender: Sender<TaskPin>) -> Self {
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

    fn submit(&mut self, server: &mut S, goal: S::Goal) -> Result<Self::TaskHandle> {
        let (outcome_sender, outcome_receiver) =
            OutcomeChannelFactory::channel::<ServerOutcome<S>>();
        let (cancel_sender, cancel_receiver) = CF::channel();

        let task_with_ctx = server.create(goal);
        let task = task_with_ctx.task;
        let feedback_receiver = task_with_ctx.feedback_receiver;
        let mut snapshot_receiver = task_with_ctx.task_state_snapshot_receiver;

        let task = async move {
            let outcome = tokio::select! {
                () = cancel_receiver.recv() => {
                    let snapshot = snapshot_receiver.recv();
                    Outcome::Cancelled(snapshot)
                }
                r = task => r,
            };
            let _ = outcome_sender.send(outcome);
        };

        self.task_sender
            .try_send(Box::pin(task))
            .map_err(|_| Error::FullTaskQueue)?;

        Ok(Self::TaskHandle::new(
            outcome_receiver,
            cancel_sender,
            feedback_receiver,
        ))
    }
}

pub struct StatefulGoalSubmitter<S, CF> {
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

    fn submit(&mut self, server: &mut S, goal: S::Goal) -> Result<Self::TaskHandle> {
        let handle = self.submitter.submit(server, goal)?;
        Ok(StatefulTaskHandle::new(handle))
    }
}

struct OutcomeChannelFactory;

impl OutcomeChannelFactory {
    fn channel<O>() -> (oneshot::Sender<O>, oneshot::Receiver<O>) {
        oneshot::channel()
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
        // Error only occurs if sender has been dropped.
        // However this can happen only if the client is not interested in cancelling the task,
        // hence at this stage the cancel capability is downgraded into NoCancel
        if self.await.is_err() {
            let downgraded = NoCancelReceiver {};
            downgraded.recv().await;
        }
    }
}

pub struct NoCancelReceiver;

impl CancelReceiver for NoCancelReceiver {
    async fn recv(self) {
        std::future::pending::<()>().await;
    }
}

/// Cancellation-disabled channel factory
pub struct NoCancelChannel;

impl CancelChannelFactory for NoCancelChannel {
    type Sender = NoCancel;
    fn channel() -> (Self::Sender, impl CancelReceiver) {
        (NoCancel, NoCancelReceiver)
    }
}

/// Cancellation-enabled channel factory (one-shot cancel)
pub struct CancelChannel;

impl CancelChannelFactory for CancelChannel {
    type Sender = WithCancel<oneshot::Sender<()>>;
    fn channel() -> (Self::Sender, impl CancelReceiver) {
        let (sender, receiver) = oneshot::channel();
        (WithCancel::new(sender), receiver)
    }
}
