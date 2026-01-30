use std::pin::Pin;

///TEX: task execution context
pub enum Outcome<S, TEX, F> {
    Succeed(S),
    Cancelled(TEX),
    Failed(F),
}

pub type PinnedOutcomeFut<S, C, F> =
    Pin<Box<dyn Future<Output = Outcome<S, C, F>> + Send + 'static>>;

pub trait ServerConcept {
    type Goal;
    type SucceedOutput;
    type CancelledOutput;
    type FailedOutput;

    type FeedbackConfig: FeedbackConfigMarker + Send + 'static;
    type TaskStateConfig: TaskStateSnapshotReceiver<TaskStateSnapshot = Self::CancelledOutput>
        + Send
        + 'static;

    #[allow(clippy::type_complexity)]
    fn create(
        &mut self,
        goal: Self::Goal,
    ) -> TaskWithContext<
        PinnedOutcomeFut<Self::SucceedOutput, Self::CancelledOutput, Self::FailedOutput>,
        Self::FeedbackConfig,
        Self::TaskStateConfig,
    >;
}

//@todo needs sealing as this should not be visible outside
pub trait FeedbackConfigMarker {}
pub struct NoFeedback;
impl FeedbackConfigMarker for NoFeedback {}

pub struct WithFeedback<R>(pub R);
pub trait FeedbackReceiverMarker {}

impl<R> FeedbackConfigMarker for WithFeedback<R> where R: FeedbackReceiverMarker {}

// concrete external receivers implementations
impl<T> FeedbackReceiverMarker for tokio::sync::watch::Receiver<T> {}

//@todo needs sealing as this should not be visible outside
pub trait TaskStateSnapshotReceiver {
    type TaskStateSnapshot;
    fn recv(&mut self) -> Self::TaskStateSnapshot;
}
pub struct NoTaskStateSnapshot;
impl TaskStateSnapshotReceiver for NoTaskStateSnapshot {
    type TaskStateSnapshot = ();
    fn recv(&mut self) -> Self::TaskStateSnapshot {}
}

pub struct WithTaskStateSnapshot<R>(pub R);

impl<R> TaskStateSnapshotReceiver for WithTaskStateSnapshot<R>
where
    R: TaskStateSnapshotReceiver,
{
    type TaskStateSnapshot = R::TaskStateSnapshot;
    fn recv(&mut self) -> Self::TaskStateSnapshot {
        self.0.recv()
    }
}

// concrete external receivers implementations
impl<T> TaskStateSnapshotReceiver for tokio::sync::watch::Receiver<T>
where
    T: Clone,
{
    type TaskStateSnapshot = T;
    fn recv(&mut self) -> Self::TaskStateSnapshot {
        let val = self.borrow_and_update();
        val.clone()
    }
}

pub struct TaskWithContext<T, FR, TR> {
    pub(crate) task: T,
    pub(crate) feedback_receiver: FR,
    pub(crate) task_state_snapshot_receiver: TR,
}

// 1) task only
impl<T> TaskWithContext<T, NoFeedback, NoTaskStateSnapshot> {
    pub fn new(task: T) -> Self {
        Self {
            task,
            feedback_receiver: NoFeedback,
            task_state_snapshot_receiver: NoTaskStateSnapshot,
        }
    }
}

// 2) task + feedback
impl<T, R> TaskWithContext<T, WithFeedback<R>, NoTaskStateSnapshot> {
    pub fn new(task: T, feedback_receiver: WithFeedback<R>) -> Self {
        Self {
            task,
            feedback_receiver,
            task_state_snapshot_receiver: NoTaskStateSnapshot,
        }
    }
}

// 3) task + task_state
impl<T, R> TaskWithContext<T, NoFeedback, WithTaskStateSnapshot<R>> {
    pub fn new(task: T, task_state_snapshot_receiver: WithTaskStateSnapshot<R>) -> Self {
        Self {
            task,
            feedback_receiver: NoFeedback,
            task_state_snapshot_receiver,
        }
    }
}

// 4) task + feedback + task_state
impl<T, FR, TR> TaskWithContext<T, WithFeedback<FR>, WithTaskStateSnapshot<TR>> {
    pub fn new(
        task: T,
        feedback_receiver: WithFeedback<FR>,
        task_state_snapshot_receiver: WithTaskStateSnapshot<TR>,
    ) -> Self {
        Self {
            task,
            feedback_receiver,
            task_state_snapshot_receiver,
        }
    }
}

pub struct StatefulServer;

pub trait VisitOutcome {
    type SucceedOutput;
    type CancelledOutput;
    type FailedOutput;
    //@todo add Result

    // by default do nothing unless explicitly implemented by the user
    fn on_succeed(&mut self, _o: &Self::SucceedOutput) {}
    fn on_cancelled(&mut self, _o: &Self::CancelledOutput) {}
    fn on_failed(&mut self, _o: &Self::FailedOutput) {}

    fn visit(
        &mut self,
        outcome: &Outcome<Self::SucceedOutput, Self::CancelledOutput, Self::FailedOutput>,
    ) {
        match outcome {
            Outcome::Succeed(s) => self.on_succeed(s),
            Outcome::Cancelled(c) => self.on_cancelled(c),
            Outcome::Failed(f) => self.on_failed(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{builder::Builder, scheduling::ScheduleTask};

    use super::*;

    #[derive(Default)]
    pub struct MyGoal {
        pub target: String,
    }

    pub struct MySucceedOutput {
        result: i32,
    }

    pub struct MyFailedOutput {
        error: String,
    }

    pub struct MyServer;
    type MyOutcome = Outcome<MySucceedOutput, NoTaskStateSnapshot, MyFailedOutput>;

    impl ServerConcept for MyServer {
        type Goal = MyGoal;
        type SucceedOutput = MySucceedOutput;
        type CancelledOutput = ();
        type FailedOutput = MyFailedOutput;

        type FeedbackConfig = NoFeedback;
        type TaskStateConfig = NoTaskStateSnapshot;

        fn create(
            &mut self,
            goal: Self::Goal,
        ) -> TaskWithContext<
            PinnedOutcomeFut<Self::SucceedOutput, Self::CancelledOutput, Self::FailedOutput>,
            NoFeedback,
            NoTaskStateSnapshot,
        > {
            let task = async move {
                let result = do_work(&goal.target).await;
                Outcome::Succeed(MySucceedOutput { result })
            };

            //@todo find a way that allows to avoid writing types here
            TaskWithContext::<_, NoFeedback, NoTaskStateSnapshot>::new(Box::pin(task))
        }
    }

    async fn do_work(_target: &str) -> i32 {
        42
    }

    // #[tokio::test]
    // async fn test_build() {
    //     let mut server = MyServer {};
    //     let (mut scheduler, executor) = Builder::with_simple_task::<MyOutcome, MyServer>();

    //     let exec_task = async move {
    //         let mut executor = executor;
    //         executor.execute().await
    //     };
    //     tokio::spawn(exec_task);
    //     let handle = scheduler.schedule(&mut server, MyGoal::default()).unwrap();
    //     let _response = handle.await_response().await;
    // }
}
