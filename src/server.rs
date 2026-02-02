#[cfg(test)]
use mockall::{automock, mock};

use std::pin::Pin;

///TEX: task execution context
#[derive(Debug)]
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

#[cfg_attr(
    test,
    automock(
        type Goal =  ();
        type SucceedOutput = ();
        type FailedOutput = ();
        type FeedbackConfig = NoFeedback;
        type TaskStateConfig = NoTaskStateSnapshot;
    )
)]
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

pub type ServerTask<S> = TaskWithContext<
    PinnedOutcomeFut<S>,
    <S as ServerConcept>::FeedbackConfig,
    <S as ServerConcept>::TaskStateConfig,
>;

pub struct TaskWithContext<T, FR, TR> {
    pub(crate) task: T,
    pub(crate) feedback_receiver: FR,
    pub(crate) task_state_snapshot_receiver: TR,
}

impl<T> TaskWithContext<T, NoFeedback, NoTaskStateSnapshot> {
    pub fn new(task: T) -> Self {
        Self {
            task,
            feedback_receiver: NoFeedback,
            task_state_snapshot_receiver: NoTaskStateSnapshot,
        }
    }
}

impl<T, R> TaskWithContext<T, WithFeedback<R>, NoTaskStateSnapshot> {
    pub fn new(task: T, feedback_receiver: WithFeedback<R>) -> Self {
        Self {
            task,
            feedback_receiver,
            task_state_snapshot_receiver: NoTaskStateSnapshot,
        }
    }
}

impl<T, R> TaskWithContext<T, NoFeedback, WithTaskStateSnapshot<R>> {
    pub fn new(task: T, task_state_snapshot_receiver: WithTaskStateSnapshot<R>) -> Self {
        Self {
            task,
            feedback_receiver: NoFeedback,
            task_state_snapshot_receiver,
        }
    }
}

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
mock! {
    pub VisitOutcomeMock {}

    impl ServerConcept for VisitOutcomeMock {
        type Goal = ();
        type SucceedOutput = ();
        type FailedOutput = ();
        type FeedbackConfig = NoFeedback;
        type TaskStateConfig = NoTaskStateSnapshot;

        fn create(&mut self, goal: <MockVisitOutcomeMock as ServerConcept>::Goal) -> ServerTask<Self>;
    }

    impl VisitOutcome for VisitOutcomeMock {
        fn on_succeed(&mut self, o: &<MockVisitOutcomeMock as ServerConcept>::SucceedOutput);
        fn on_cancelled(&mut self, o: &ServerSnapshot<Self>);
        fn on_failed(&mut self, o: &<MockVisitOutcomeMock as ServerConcept>::FailedOutput);
        fn visit(&mut self, outcome: &ServerOutcome<Self>);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        factory::Factory,
        submitting::{CancelChannel, NoCancelChannel, SubmitGoal},
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

    pub struct TestServerA;

    impl ServerConcept for TestServerA {
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

    async fn do_work_a(_target: &str) -> i32 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        42
    }

    pub struct MySucceedOutputB {
        bytes: usize,
    }
    pub struct MyFailedOutputB {
        code: u32,
    }

    pub struct TestServerB;

    impl ServerConcept for TestServerB {
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

    async fn do_work_b(target: &str) -> usize {
        target.len()
    }

    // compile-time test
    #[test]
    fn two_servers_impl_same_goal_test() {
        let (_s1, _e1) = Factory::stateless::<TestServerA, NoCancelChannel>();
        let (_s2, _e2) = Factory::stateless::<TestServerB, NoCancelChannel>();
    }

    #[tokio::test]
    async fn no_request_cancel_test() {
        let mut server = TestServerA {};
        let (mut submitter, mut executor) = Factory::stateless::<TestServerA, CancelChannel>();
        let handle = submitter.submit(&mut server, MyGoal::default()).unwrap();
        let result_handle = handle.into_result_receiver();

        tokio::spawn(async move { executor.execute().await });
        let out = result_handle.await.unwrap();
        assert!(matches!(out, Outcome::Succeed(_)));
    }

    #[tokio::test]
    async fn request_cancel_test() {
        let mut server = TestServerA {};
        let (mut submitter, mut executor) = Factory::stateless::<TestServerA, CancelChannel>();
        let handle = submitter.submit(&mut server, MyGoal::default()).unwrap();
        let (result_handle, _) = handle.cancel();

        tokio::spawn(async move { executor.execute().await });
        let out = result_handle.await.unwrap();
        assert!(matches!(out, Outcome::Cancelled(_)));
    }

    #[derive(Clone, Debug, Default)]
    pub struct MyTaskState {
        pub progress: u32,
    }

    #[derive(Debug)]
    pub struct MySucceedOutputC;
    #[derive(Debug)]
    pub struct MyFailedOutputC;

    #[derive(Debug, Default)]
    pub struct MyProgressGoal {
        initial_progress: u32,
    }

    pub struct TestServerC;

    impl ServerConcept for TestServerC {
        type Goal = MyProgressGoal;
        type SucceedOutput = MySucceedOutputC;
        type FailedOutput = MyFailedOutputC;

        type FeedbackConfig = NoFeedback;
        type TaskStateConfig = WithTaskStateSnapshot<tokio::sync::watch::Receiver<MyTaskState>>;

        fn create(&mut self, goal: Self::Goal) -> ServerTask<Self> {
            let (state_sender, state_receiver) = tokio::sync::watch::channel(MyTaskState {
                progress: goal.initial_progress,
            });

            let task = async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                state_sender.send(MyTaskState { progress: 50 }).unwrap();
                tokio::time::sleep(Duration::from_millis(100)).await;
                state_sender.send(MyTaskState { progress: 100 }).unwrap();
                Outcome::Succeed(MySucceedOutputC)
            };

            ServerTask::<Self>::new(Box::pin(task), WithTaskStateSnapshot(state_receiver))
        }
    }

    #[tokio::test]
    async fn cancel_with_exe_ctx_test() {
        let mut server = TestServerC {};
        let (mut submitter, mut executor) = Factory::stateless::<TestServerC, CancelChannel>();

        let handle = submitter
            .submit(&mut server, MyProgressGoal::default())
            .unwrap();
        let (result_handle, _) = handle.cancel();
        tokio::select! {
            _ = executor.execute() => {},
            result = result_handle => {
                let result = result.unwrap();
                assert!(matches!(result, Outcome::Cancelled(MyTaskState { progress: 0})));
            }
        };

        let handle = submitter
            .submit(&mut server, MyProgressGoal::default())
            .unwrap();
        let result = tokio::time::timeout(Duration::from_millis(100), executor.execute()).await;
        assert!(result.is_err());
        let (result_handle, _) = handle.cancel();

        tokio::select! {
            _ = executor.execute() => {},
            result = result_handle => {
                let result = result.unwrap();
                assert!(matches!(result, Outcome::Cancelled(MyTaskState { progress: 50})));
            }
        };
    }

    #[tokio::test]
    async fn stateful_server_visit_called() {
        let mut mock_server = MockVisitOutcomeMock::new();
        mock_server.expect_create().once().return_once(|()| {
            ServerTask::<MockVisitOutcomeMock>::new(Box::pin(async { Outcome::Succeed(()) }))
        });
        mock_server.expect_visit().times(1).returning(|outcome| {
            assert!(matches!(outcome, Outcome::Succeed(_)));
        });

        let (mut submitter, mut executor) =
            Factory::stateful::<MockVisitOutcomeMock, CancelChannel>();

        tokio::spawn(async move {
            executor.execute().await;
        });

        let handle = submitter.submit(&mut mock_server, ()).unwrap();
        let visitable_result = handle.into_visitable_result(&mut mock_server);
        let _ = visitable_result.await_result().await;
    }
}
