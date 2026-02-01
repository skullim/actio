use std::pin::Pin;

///TEX: task execution context
pub enum Outcome<S, TEX, F> {
    Succeed(S),
    Cancelled(TEX),
    Failed(F),
}

pub type ServerOutcome<S> = Outcome<
    <S as ServerConcept>::SucceedOutput,
    ServerSnapshot<S>,
    <S as ServerConcept>::FailedOutput,
>;

pub type ServerSnapshot<S> =
    <<S as ServerConcept>::TaskStateConfig as TaskStateSnapshotReceiver>::TaskStateSnapshot;

pub type PinnedOutcomeFut<S> = Pin<Box<dyn Future<Output = ServerOutcome<S>> + Send + 'static>>;

pub type ServerTask<S> = TaskWithContext<
    PinnedOutcomeFut<S>,
    <S as ServerConcept>::FeedbackConfig,
    <S as ServerConcept>::TaskStateConfig,
>;

pub trait ServerConcept {
    type Goal: Send + 'static;
    type SucceedOutput: Send + 'static;
    type FailedOutput: Send + 'static;

    type FeedbackConfig: FeedbackConfigMarker + Send + 'static;
    type TaskStateConfig: TaskStateSnapshotReceiver + Send + 'static;

    fn create(&mut self, goal: Self::Goal) -> ServerTask<Self>;
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
    type TaskStateSnapshot: Send + 'static;
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
    T: Clone + Send + 'static,
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

pub trait VisitOutcome: ServerConcept {
    // user implements methods that is interested in
    fn on_succeed(&mut self, _o: &Self::SucceedOutput) {}
    fn on_cancelled(&mut self, _o: &ServerSnapshot<Self>) {}
    fn on_failed(&mut self, _o: &Self::FailedOutput) {}

    //@todo add Result
    fn visit(&mut self, outcome: &ServerOutcome<Self>) {
        match outcome {
            Outcome::Succeed(s) => self.on_succeed(s),
            Outcome::Cancelled(c) => self.on_cancelled(c),
            Outcome::Failed(f) => self.on_failed(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        factory::Factory,
        submitting::{NoCancelChannel, SubmitGoal},
    };

    #[derive(Default)]
    pub struct MyGoal {
        pub target: String,
    }

    pub struct MySucceedOutputA {
        result: i32,
    }
    pub struct MyFailedOutputA {
        error: String,
    }

    pub struct MySucceedOutputB {
        bytes: usize,
    }
    pub struct MyFailedOutputB {
        code: u32,
    }

    pub struct MyServerA;
    pub struct MyServerB;

    impl ServerConcept for MyServerA {
        type Goal = MyGoal;
        type SucceedOutput = MySucceedOutputA;
        type FailedOutput = MyFailedOutputA;

        type FeedbackConfig = NoFeedback;
        type TaskStateConfig = NoTaskStateSnapshot;

        fn create(&mut self, goal: Self::Goal) -> ServerTask<Self> {
            let task = async move {
                let result = do_work_a(&goal.target).await;
                Outcome::Succeed(MySucceedOutputA { result })
            };
            ServerTask::<Self>::new(Box::pin(task))
        }
    }

    impl ServerConcept for MyServerB {
        type Goal = MyGoal;
        type SucceedOutput = MySucceedOutputB;
        type FailedOutput = MyFailedOutputB;

        type FeedbackConfig = NoFeedback;
        type TaskStateConfig = NoTaskStateSnapshot;

        fn create(&mut self, goal: Self::Goal) -> ServerTask<Self> {
            let task = async move {
                let bytes = do_work_b(&goal.target).await;
                Outcome::Succeed(MySucceedOutputB { bytes })
            };
            ServerTask::<Self>::new(Box::pin(task))
        }
    }

    async fn do_work_a(_target: &str) -> i32 {
        42
    }
    async fn do_work_b(target: &str) -> usize {
        target.len()
    }

    #[tokio::test]
    async fn test_two_servers_same_goal() {
        let mut server_a = MyServerA {};
        let (mut submitter_a, executor_a) = Factory::instantiate::<MyServerA, NoCancelChannel>();

        tokio::spawn(async move {
            let mut executor_a = executor_a;
            executor_a.execute().await
        });

        let handle_a = submitter_a
            .submit(&mut server_a, MyGoal::default())
            .unwrap();
        let out_a = handle_a.into_result_receiver().await.unwrap();

        match out_a {
            Outcome::Succeed(_s) => {}
            Outcome::Cancelled(_) => panic!("unexpected cancel"),
            Outcome::Failed(_) => panic!("unexpected failure"),
        }

        let mut server_b = MyServerB {};
        let (mut submitter_b, executor_b) = Factory::instantiate::<MyServerB, NoCancelChannel>();

        tokio::spawn(async move {
            let mut executor_b = executor_b;
            executor_b.execute().await
        });

        let handle_b = submitter_b
            .submit(&mut server_b, MyGoal::default())
            .unwrap();
        let out_b = handle_b.into_result_receiver().await.unwrap();

        match out_b {
            Outcome::Succeed(_s) => {}
            Outcome::Cancelled(_) => panic!("unexpected cancel"),
            Outcome::Failed(_) => panic!("unexpected failure"),
        }
    }
}
