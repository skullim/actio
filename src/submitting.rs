use std::marker::PhantomData;

use crate::server::{
    Outcome, ServerOutcome, TaskStateSnapshotReceiver, VisitOutcome, WithFeedback,
};
use crate::{PinnedTask, server::ServerConcept};
use anyhow::anyhow;
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

pub struct GoalSubmitter<S, CF> {
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
    type TaskHandle = PubTaskHandle<S, CF>;

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

        Ok(TaskHandle {
            result_receiver,
            cancel_sender,
            feedback_receiver,
        })
    }
}

struct ResultChannelFactory;

impl ResultChannelFactory {
    fn channel<O>() -> (oneshot::Sender<O>, oneshot::Receiver<O>) {
        tokio::sync::oneshot::channel()
    }
}

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

pub type PubTaskHandle<S, CF> = TaskHandle<
    tokio::sync::oneshot::Receiver<ServerOutcome<S>>,
    <CF as CancelChannelFactory>::Sender,
    <S as ServerConcept>::FeedbackConfig,
>;

pub struct TaskHandle<R, C, F> {
    result_receiver: R,
    cancel_sender: C,
    feedback_receiver: F,
}

impl<R, C, F> TaskHandle<R, C, F> {
    pub fn result_receiver(&self) -> &R {
        &self.result_receiver
    }

    pub fn result_receiver_mut(&mut self) -> &mut R {
        &mut self.result_receiver
    }

    pub fn into_result_receiver(self) -> R {
        self.result_receiver
    }
}
pub struct NoCancel;

// do not expose the sender: cancelling is one-shot action that is guarded by SendCancel trait
pub struct WithCancel<S>(S);
pub trait CancelConfigMarker {}

impl CancelConfigMarker for NoCancel {}

pub trait CancelSenderMarker {}
pub trait SendCancel {
    fn send(self) -> anyhow::Result<()>;
}

impl<T> CancelSenderMarker for tokio::sync::oneshot::Sender<T> {}

impl SendCancel for tokio::sync::oneshot::Sender<()> {
    fn send(self) -> anyhow::Result<()> {
        //@todo check if error should be handled. Failing means other side dropped, so in theory only on Full there should be meaningful error
        self.send(())
            .map_err(|_| anyhow!("failed to send cancel request"))
    }
}

impl<S> CancelConfigMarker for WithCancel<S> where S: CancelSenderMarker + SendCancel {}

// if cancel is present
impl<R, S, F> TaskHandle<R, WithCancel<S>, F>
where
    S: SendCancel,
{
    pub fn cancel(self) -> (R, F) {
        // can fail only if channel full, but since we sent and consume it should never happen
        let _ = self.cancel_sender.0.send();
        (self.result_receiver, self.feedback_receiver)
    }
}

// if feedback is present
impl<R, C, Rx> TaskHandle<R, C, WithFeedback<Rx>> {
    pub fn feedback_receiver(&self) -> &WithFeedback<Rx> {
        &self.feedback_receiver
    }

    pub fn feedback_receiver_mut(&mut self) -> &mut WithFeedback<Rx> {
        &mut self.feedback_receiver
    }

    pub fn into_parts(self) -> (R, WithFeedback<Rx>) {
        (self.result_receiver, self.feedback_receiver)
    }
}

pub trait CancelChannelFactory {
    type Sender;
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
    type Sender = ();
    fn channel() -> (Self::Sender, impl CancelReceiver) {
        ((), NoCancelReceiver)
    }
}

pub struct CancelChannel;

impl CancelChannelFactory for CancelChannel {
    type Sender = oneshot::Sender<()>;
    fn channel() -> (Self::Sender, impl CancelReceiver) {
        tokio::sync::oneshot::channel()
    }
}

pub struct StatefulTaskHandle<S, CF>
where
    S: ServerConcept,
    CF: CancelChannelFactory,
{
    handle: PubTaskHandle<S, CF>,
}

impl<S, CF> StatefulTaskHandle<S, CF>
where
    S: ServerConcept,
    CF: CancelChannelFactory,
{
    pub async fn await_result<'a>(
        self,
        server: &'a mut S,
    ) -> VisitableResult<'a, S, ServerOutcome<S>>
    where
        S: VisitOutcome,
    {
        let receiver = self.handle.into_result_receiver();
        VisitableResult { server, receiver }
    }
}

pub struct VisitableResult<'a, S, O> {
    server: &'a mut S,
    receiver: oneshot::Receiver<O>,
}

impl<'a, S> VisitableResult<'a, S, ServerOutcome<S>>
where
    S: VisitOutcome,
{
    pub async fn await_result(self) -> ServerOutcome<S> {
        let outcome = self.receiver.await.unwrap();
        self.server.visit(&outcome);
        outcome
    }
}
