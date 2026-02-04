use std::pin::Pin;
use tokio::sync::watch;

///TEX: task execution context
#[derive(Debug)]
pub enum Outcome<S, TEX, F> {
    Succeed(S),
    Cancelled(TEX),
    Failed(F),
}

pub type ServerOutcome<S> =
    Outcome<<S as ServerConcept>::Succeed, ServerSnapshot<S>, <S as ServerConcept>::Failed>;

pub type ServerSnapshot<S> =
    <<S as ServerConcept>::TaskState as TaskStateSnapshotReceiver>::Snapshot;

pub type OutcomeFutPin<S> = Pin<Box<dyn Future<Output = ServerOutcome<S>> + Send + 'static>>;

pub trait ServerConcept {
    type Goal: Send + 'static;
    type Succeed: Send + 'static;
    type Failed: Send + 'static;

    type Feedback: FeedbackMarker + Send + 'static;
    type TaskState: TaskStateSnapshotReceiver + Send + 'static;

    fn create(&mut self, goal: Self::Goal) -> ServerTask<Self>;
}

pub trait FeedbackMarker: sealed::Sealed {}
mod sealed {
    pub trait Sealed {}
}

pub struct NoFeedback;
impl sealed::Sealed for NoFeedback {}

impl FeedbackMarker for NoFeedback {}

pub struct WithFeedback<R>(pub R);
impl<R> sealed::Sealed for WithFeedback<R> {}

pub type WithFeedbackWatch<T> = WithFeedback<watch::Receiver<T>>;

pub trait FeedbackReceiverMarker {}

impl<R> FeedbackMarker for WithFeedback<R> where R: FeedbackReceiverMarker {}

// concrete external receivers implementations
impl<T> FeedbackReceiverMarker for watch::Receiver<T> {}

pub trait TaskStateSnapshotReceiver {
    type Snapshot: Send + 'static;
    fn recv(&mut self) -> Self::Snapshot;
}
pub struct NoTaskStateSnapshot;
impl TaskStateSnapshotReceiver for NoTaskStateSnapshot {
    type Snapshot = ();
    fn recv(&mut self) -> Self::Snapshot {}
}

// concrete external receivers implementations
impl<T> TaskStateSnapshotReceiver for watch::Receiver<T>
where
    T: Clone + Send + 'static,
{
    type Snapshot = T;

    fn recv(&mut self) -> Self::Snapshot {
        let val = self.borrow_and_update();
        val.clone()
    }
}

pub struct WithTaskStateSnapshot<R>(pub R);

pub type WithTaskStateSnapshotWatch<T> = WithTaskStateSnapshot<watch::Receiver<T>>;

impl<R> TaskStateSnapshotReceiver for WithTaskStateSnapshot<R>
where
    R: TaskStateSnapshotReceiver,
{
    type Snapshot = R::Snapshot;
    fn recv(&mut self) -> Self::Snapshot {
        self.0.recv()
    }
}

pub type ServerTask<S> = TaskWithContext<
    OutcomeFutPin<S>,
    <S as ServerConcept>::Feedback,
    <S as ServerConcept>::TaskState,
>;

pub struct TaskWithContext<T, FR, TR> {
    pub(crate) task: T,
    pub(crate) feedback_receiver: FR,
    pub(crate) task_state_snapshot_receiver: TR,
}

impl<T, FR, TR> TaskWithContext<T, FR, TR> {
    pub fn new(task: T) -> TaskWithContext<T, NoFeedback, NoTaskStateSnapshot> {
        TaskWithContext {
            task,
            feedback_receiver: NoFeedback,
            task_state_snapshot_receiver: NoTaskStateSnapshot,
        }
    }
}

impl<T, FR, TR> TaskWithContext<T, FR, TR> {
    pub fn with_feedback<R>(self, feedback_receiver: R) -> TaskWithContext<T, WithFeedback<R>, TR>
    where
        R: FeedbackReceiverMarker,
    {
        TaskWithContext {
            task: self.task,
            feedback_receiver: WithFeedback(feedback_receiver),
            task_state_snapshot_receiver: self.task_state_snapshot_receiver,
        }
    }
}

impl<T, FR, TR> TaskWithContext<T, FR, TR> {
    pub fn with_task_state<R>(
        self,
        task_state_snapshot_receiver: R,
    ) -> TaskWithContext<T, FR, WithTaskStateSnapshot<R>>
    where
        R: TaskStateSnapshotReceiver,
    {
        TaskWithContext {
            task: self.task,
            feedback_receiver: self.feedback_receiver,
            task_state_snapshot_receiver: WithTaskStateSnapshot(task_state_snapshot_receiver),
        }
    }
}

pub trait VisitOutcome: ServerConcept {
    type Error;
    // user implements methods that is interested in
    fn on_succeed(&mut self, _o: &Self::Succeed) -> Result<(), Self::Error> {
        Ok(())
    }
    fn on_cancelled(&mut self, _o: &ServerSnapshot<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
    fn on_failed(&mut self, _o: &Self::Failed) -> Result<(), Self::Error> {
        Ok(())
    }

    fn visit(&mut self, outcome: &ServerOutcome<Self>) -> Result<(), Self::Error> {
        match outcome {
            Outcome::Succeed(s) => self.on_succeed(s),
            Outcome::Cancelled(c) => self.on_cancelled(c),
            Outcome::Failed(f) => self.on_failed(f),
        }
    }
}
