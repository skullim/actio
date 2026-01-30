use std::marker::PhantomData;

use crate::server::{Outcome, TaskStateSnapshotReceiver, VisitOutcome, WithFeedback};
use crate::{PinnedTask, server::ServerConcept};
use anyhow::anyhow;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum Error {
    Full,
    RejectedByExecutor,
}

pub trait ScheduleTask<G, S> {
    type TaskHandle;

    fn schedule(&mut self, server: &mut S, goal: G) -> Result<Self::TaskHandle, Error>;
}

pub struct GenericScheduler<A, RF, CF> {
    task_sender: Sender<PinnedTask>,
    phantom: PhantomData<(A, RF, CF)>,
}

impl<A, RF, CF> GenericScheduler<A, RF, CF>
where
    A: ActionInterface,
{
    pub fn new(task_sender: Sender<PinnedTask>) -> Self {
        Self {
            task_sender,
            phantom: PhantomData,
        }
    }
}

//@todo would be better if higher layer already provides outcome = Outcome<A::Succeed, A::Cancelled, A::Failed>;
impl<A, S, RF, CF> ScheduleTask<A::Goal, S> for GenericScheduler<A, RF, CF>
where
    A: ActionInterface,
    S: ServerConcept<
            Goal = A::Goal,
            SucceedOutput = A::Succeed,
            CancelledOutput = A::Cancelled,
            FailedOutput = A::Failed,
        >,
    RF: MandatoryChannelFactory<Outcome<A::Succeed, A::Cancelled, A::Failed>>,
    CF: CancelChannelFactory<Output = A::Cancelled>,
{
    type TaskHandle = TaskHandle<
        oneshot::Receiver<Outcome<A::Succeed, A::Cancelled, A::Failed>>,
        tokio::sync::mpsc::Sender<A::Cancelled>,
        S::FeedbackConfig,
    >;
    fn schedule(&mut self, server: &mut S, goal: A::Goal) -> Result<Self::TaskHandle, Error> {
        let (result_sender, result_receiver) = RF::channel();
        let (cancel_sender, mut cancel_receiver) = CF::channel();

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
                r = task => { r }
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

pub trait ActionInterface {
    type Goal;
    type Succeed: 'static + Send;
    type Cancelled: 'static + Send;
    type Failed: 'static + Send;
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

pub trait MandatoryChannelFactory<O> {
    //@todo use traits for sender and receiver and return impl pair
    fn channel() -> (oneshot::Sender<O>, oneshot::Receiver<O>);
}

pub trait CancelChannelFactory {
    type Output;
    #[allow(clippy::type_complexity)]
    fn channel() -> (Sender<Self::Output>, Receiver<Self::Output>);
}

pub struct CancelChannel;

impl CancelChannelFactory for CancelChannel {
    type Output = ();
    fn channel() -> (Sender<Self::Output>, Receiver<Self::Output>) {
        tokio::sync::mpsc::channel(1)
    }
}

pub trait AwaitResult {
    type Result;
}

pub trait AwaitVisitableResult {
    type Result;
    type Visitor: VisitOutcome;
    fn await_and_visit(&self, visitor: &mut Self::Visitor) -> Self::Result;
}
