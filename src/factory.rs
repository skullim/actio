use futures::channel::mpsc::channel;

use crate::{
    execution::Executor,
    server::ServerConcept,
    submitting::{CancelChannelFactory, GoalSubmitter, StatefulGoalSubmitter, SubmitGoal},
    task_handle::{StatefulTaskHandle, TaskHandle},
};

pub struct Factory;

impl Factory {
    pub fn stateless<S, CF>(
        task_queue_buffer: usize,
    ) -> (
        impl SubmitGoal<S::Goal, Server = S, TaskHandle = TaskHandle<S, CF>>,
        Executor,
    )
    where
        S: ServerConcept,
        CF: CancelChannelFactory,
    {
        let (task_sender, task_receiver) = channel(task_queue_buffer);

        (
            GoalSubmitter::<S, CF>::new(task_sender),
            Executor::new(task_receiver),
        )
    }

    pub fn stateful<S, CF>(
        task_queue_buffer: usize,
    ) -> (
        impl SubmitGoal<S::Goal, Server = S, TaskHandle = StatefulTaskHandle<S, CF>>,
        Executor,
    )
    where
        S: ServerConcept,
        CF: CancelChannelFactory,
    {
        let (task_sender, task_receiver) = channel(task_queue_buffer);
        let submitter = GoalSubmitter::<S, CF>::new(task_sender);

        (
            StatefulGoalSubmitter::<S, CF>::new(submitter),
            Executor::new(task_receiver),
        )
    }
}
