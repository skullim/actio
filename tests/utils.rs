use actio::{Executor, ServerConcept, ServerOutcome};

pub const STANDARD_TASK_QUEUE_SIZE: usize = 16;
pub const STRESS_TEST_TASK_QUEUE_SIZE: usize = 10_000;

pub async fn await_outcome<S>(
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

pub async fn poll_executor_for(duration: tokio::time::Duration, executor: &mut Executor) {
    let _ = tokio::time::timeout(duration, executor.execute()).await;
}

pub mod impls {
    #[derive(Default)]
    pub struct Goal {
        pub target: String,
    }

    pub mod a {
        use std::time::Duration;

        use actio::{NoFeedback, NoTaskStateSnapshot, Outcome, ServerConcept, ServerTask};

        pub struct SucceedOutput {
            result: usize,
        }
        pub struct FailedOutput {
            error: String,
        }

        pub struct TestServer;

        impl ServerConcept for TestServer {
            type Goal = super::Goal;
            type Succeed = SucceedOutput;
            type Failed = FailedOutput;

            type Feedback = NoFeedback;
            type TaskState = NoTaskStateSnapshot;

            fn create(&mut self, goal: Self::Goal) -> ServerTask<Self> {
                let task = async move {
                    let result = do_work(&goal.target).await;
                    Outcome::Succeed(SucceedOutput { result })
                };
                ServerTask::<Self>::new(Box::pin(task))
            }
        }

        async fn do_work(target: &str) -> usize {
            tokio::time::sleep(Duration::from_millis(10)).await;
            target.len()
        }
    }

    pub mod b {
        use actio::{NoFeedback, NoTaskStateSnapshot, Outcome, ServerConcept, ServerTask};

        pub struct MySucceedOutput {
            bytes: usize,
        }
        pub struct MyFailedOutput {
            code: u32,
        }

        pub struct TestServer;

        impl ServerConcept for TestServer {
            type Goal = super::Goal;
            type Succeed = MySucceedOutput;
            type Failed = MyFailedOutput;

            type Feedback = NoFeedback;
            type TaskState = NoTaskStateSnapshot;

            fn create(&mut self, goal: Self::Goal) -> ServerTask<Self> {
                let task = async move {
                    let bytes = do_work(&goal.target).await;
                    Outcome::Succeed(MySucceedOutput { bytes })
                };
                ServerTask::<Self>::new(Box::pin(task))
            }
        }

        async fn do_work(target: &str) -> usize {
            target.len()
        }
    }

    pub mod c {
        use actio::{NoFeedback, Outcome, ServerConcept, ServerTask, WithTaskStateSnapshot};
        use std::time::Duration;

        #[derive(Clone, Debug, Default)]
        pub struct TaskState {
            pub progress: u32,
        }

        #[derive(Debug)]
        pub struct SucceedOutput;
        #[derive(Debug)]
        pub struct FailedOutput;

        #[derive(Debug, Default)]
        pub struct ProgressGoal {
            initial_progress: u32,
        }

        pub struct TestServer;

        impl ServerConcept for TestServer {
            type Goal = ProgressGoal;
            type Succeed = SucceedOutput;
            type Failed = FailedOutput;

            type Feedback = NoFeedback;
            type TaskState = WithTaskStateSnapshot<tokio::sync::watch::Receiver<TaskState>>;

            fn create(&mut self, goal: Self::Goal) -> ServerTask<Self> {
                let (state_sender, state_receiver) = tokio::sync::watch::channel(TaskState {
                    progress: goal.initial_progress,
                });

                let task = async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    state_sender.send(TaskState { progress: 50 }).unwrap();
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    state_sender.send(TaskState { progress: 100 }).unwrap();
                    Outcome::Succeed(SucceedOutput)
                };

                ServerTask::<Self>::new(Box::pin(task)).with_task_state(state_receiver)
            }
        }
    }
}
