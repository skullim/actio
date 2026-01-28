use tokio::sync::mpsc::channel;

use crate::{
    execution::Executor,
    scheduling::{ScheduleTask, SimpleTaskScheduler, TaskWithFeedbackScheduler},
    server::ServerConcept,
    task::{SimpleTask, TaskHandle, TaskHandleWithFeedback, TaskWithFeedback},
};

pub struct Builder;

impl Builder {
    pub fn with_simple_task<G, R, S, F>(
        server: S,
    ) -> (impl ScheduleTask<G, TaskHandle = TaskHandle<R>>, Executor)
    where
        S: ServerConcept<G, Task = SimpleTask<F>>,
        F: Future<Output = R> + 'static,
        F::Output: 'static,
    {
        let (task_sender, task_receiver) = channel(16);
        (
            SimpleTaskScheduler::<S>::new(server, task_sender),
            Executor::new(task_receiver),
        )
    }

    pub fn with_task_with_feedback<G, R, S, F, Fb>(
        server: S,
    ) -> (
        impl ScheduleTask<G, TaskHandle = TaskHandleWithFeedback<R, Fb>>,
        Executor,
    )
    where
        S: ServerConcept<G, Task = TaskWithFeedback<F, Fb>>,
        F: Future<Output = R> + 'static,
        R: 'static,
        Fb: 'static,
    {
        let (task_sender, task_receiver) = channel(16);
        (
            TaskWithFeedbackScheduler::<S>::new(server, task_sender),
            Executor::new(task_receiver),
        )
    }
}
