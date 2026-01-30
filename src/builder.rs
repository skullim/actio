use tokio::sync::mpsc::channel;

use crate::{execution::Executor, scheduling::ScheduleTask, server::ServerConcept};

pub struct Builder;

impl Builder {
    // pub fn with_simple_task<R, S>() -> (
    //     impl ScheduleTask<S::Goal, S, TaskHandle = TaskHandle<R>>,
    //     Executor,
    // )
    // where
    //     S: ServerConcept<Task = SimpleTask<R>>,
    //     R: Send + 'static,
    // {
    //     let (task_sender, task_receiver) = channel(16);
    //     (
    //         SimpleTaskScheduler::new(task_sender),
    //         Executor::new(task_receiver),
    //     )
    // }

    // pub fn with_task_with_feedback<G, R, S, F, Fb>() -> (
    //     impl ScheduleTask<G, S, TaskHandle = TaskHandleWithFeedback<R, Fb>>,
    //     Executor,
    // )
    // where
    //     S: ServerConcept<Goal = G, Task = TaskWithFeedback<F, Fb>>,
    //     F: Future<Output = R> + Send + 'static,
    //     R: Send + 'static,
    //     Fb: Send + 'static,
    // {
    //     let (task_sender, task_receiver) = channel(16);
    //     (
    //         TaskWithFeedbackScheduler::new(task_sender),
    //         Executor::new(task_receiver),
    //     )
    // }
}
