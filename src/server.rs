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
    type Error;
    // user implements methods that is interested in
    fn on_succeed(&mut self, _o: &Self::SucceedOutput) -> Result<(), Self::Error> {
        Ok(())
    }
    fn on_cancelled(&mut self, _o: &ServerSnapshot<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
    fn on_failed(&mut self, _o: &Self::FailedOutput) -> Result<(), Self::Error> {
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

#[cfg(test)]
mock! {
    pub VisitOutcome {}

    impl ServerConcept for VisitOutcome {
        type Goal = ();
        type SucceedOutput = ();
        type FailedOutput = ();
        type FeedbackConfig = NoFeedback;
        type TaskStateConfig = NoTaskStateSnapshot;

        fn create(&mut self, goal: <MockVisitOutcome as ServerConcept>::Goal) -> ServerTask<Self>;
    }

    impl VisitOutcome for VisitOutcome {
        type Error = anyhow::Error;

        fn on_succeed(&mut self, o: &<MockVisitOutcome as ServerConcept>::SucceedOutput) -> Result<(), <MockVisitOutcome as VisitOutcome>::Error>;
        fn on_cancelled(&mut self, o: &ServerSnapshot<Self>) -> Result<(), <MockVisitOutcome as VisitOutcome>::Error>;
        fn on_failed(&mut self, o: &<MockVisitOutcome as ServerConcept>::FailedOutput) -> Result<(), <MockVisitOutcome as VisitOutcome>::Error>;

        fn visit(&mut self, outcome: &ServerOutcome<Self>)-> Result<(), <MockVisitOutcome as VisitOutcome>::Error>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        execution::Executor,
        factory::Factory,
        submitting::{CancelChannel, NoCancelChannel, SubmitGoal},
    };
    use std::time::Duration;

    const STANDARD_TASK_QUEUE_SIZE: usize = 16;
    const STRESS_TEST_TASK_QUEUE_SIZE: usize = 10_000;

    #[derive(Default)]
    pub struct MyGoal {
        pub target: String,
    }

    pub struct MySucceedOutputA {
        result: usize,
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

    async fn do_work_a(target: &str) -> usize {
        tokio::time::sleep(Duration::from_millis(10)).await;
        target.len()
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

    async fn execute_till_outcome<S>(
        recv: tokio::sync::oneshot::Receiver<ServerOutcome<S>>,
        executor: &mut Executor,
    ) -> ServerOutcome<S>
    where
        S: ServerConcept,
    {
        tokio::select! {
            _ = executor.execute() => {
                panic!("executor should never finish before outcome handle")
            },
            outcome = recv => {
                outcome.unwrap()
            }
        }
    }

    async fn poll_executor_for(duration: tokio::time::Duration, executor: &mut Executor) {
        let _ = tokio::time::timeout(duration, executor.execute()).await;
    }

    // compile-time test
    #[test]
    fn two_servers_impl_same_goal_test() {
        let (_s1, _e1) =
            Factory::stateless::<TestServerA, NoCancelChannel>(STANDARD_TASK_QUEUE_SIZE);
        let (_s2, _e2) =
            Factory::stateless::<TestServerB, NoCancelChannel>(STANDARD_TASK_QUEUE_SIZE);
    }

    #[tokio::test]
    async fn no_request_cancel_test() {
        let mut server = TestServerA {};
        let (mut submitter, mut executor) =
            Factory::stateless::<TestServerA, CancelChannel>(STANDARD_TASK_QUEUE_SIZE);
        let handle = submitter.submit(&mut server, MyGoal::default()).unwrap();
        let outcome_recv = handle.into_outcome_receiver();

        let out = execute_till_outcome::<TestServerA>(outcome_recv, &mut executor).await;
        assert!(matches!(out, Outcome::Succeed(_)));
    }

    #[tokio::test]
    async fn request_cancel_test() {
        let mut server = TestServerA {};
        let (mut submitter, mut executor) =
            Factory::stateless::<TestServerA, CancelChannel>(STANDARD_TASK_QUEUE_SIZE);
        let handle = submitter.submit(&mut server, MyGoal::default()).unwrap();
        let (outcome_recv, _) = handle.cancel();

        let out = execute_till_outcome::<TestServerA>(outcome_recv, &mut executor).await;
        assert!(matches!(out, Outcome::Cancelled(_)));
    }

    #[tokio::test]
    async fn cancel_with_exe_ctx_test() {
        let mut server = TestServerC {};
        let (mut submitter, mut executor) =
            Factory::stateless::<TestServerC, CancelChannel>(STANDARD_TASK_QUEUE_SIZE);

        let handle = submitter
            .submit(&mut server, MyProgressGoal::default())
            .unwrap();
        let (outcome_recv, _) = handle.cancel();
        let out = execute_till_outcome::<TestServerC>(outcome_recv, &mut executor).await;
        assert!(matches!(
            out,
            Outcome::Cancelled(MyTaskState { progress: 0 })
        ));

        let handle = submitter
            .submit(&mut server, MyProgressGoal::default())
            .unwrap();
        poll_executor_for(Duration::from_millis(100), &mut executor).await;

        let (outcome_recv, _) = handle.cancel();

        let out = execute_till_outcome::<TestServerC>(outcome_recv, &mut executor).await;
        assert!(matches!(
            out,
            Outcome::Cancelled(MyTaskState { progress: 50 })
        ));
    }

    #[tokio::test]
    async fn stateful_server_visit_called() {
        let mut mock_server = MockVisitOutcome::new();
        mock_server.expect_create().once().return_once(|()| {
            ServerTask::<MockVisitOutcome>::new(Box::pin(async { Outcome::Succeed(()) }))
        });
        mock_server.expect_visit().times(1).returning(|_| Ok(()));

        let (mut submitter, mut executor) =
            Factory::stateful::<MockVisitOutcome, CancelChannel>(STANDARD_TASK_QUEUE_SIZE);

        let handle = submitter.submit(&mut mock_server, ()).unwrap();
        let visitable_outcome = handle.into_visitable_outcome(&mut mock_server);
        tokio::spawn(async move {
            executor.execute().await;
        });
        let _ = visitable_outcome.outcome().await;
    }

    #[tokio::test]
    async fn stress_test() {
        let mut server = TestServerC {};
        let (mut submitter, mut executor) =
            Factory::stateless::<TestServerC, CancelChannel>(STRESS_TEST_TASK_QUEUE_SIZE);
        let mut handles: Vec<_> = (0..STRESS_TEST_TASK_QUEUE_SIZE)
            .map(|_| {
                submitter
                    .submit(&mut server, MyProgressGoal::default())
                    .unwrap()
            })
            .collect();

        poll_executor_for(Duration::from_millis(42), &mut executor).await;
        let handle = handles.remove(4_200);
        let (outcome_recv, _) = handle.cancel();
        let result = tokio::time::timeout(
            Duration::from_millis(1),
            execute_till_outcome::<TestServerC>(outcome_recv, &mut executor),
        )
        .await;
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), Outcome::Cancelled(_)));
    }
}
