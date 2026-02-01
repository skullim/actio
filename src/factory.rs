use tokio::sync::mpsc::channel;

use crate::{
    execution::Executor,
    server::ServerConcept,
    submitting::{CancelChannelFactory, GoalSubmitter, PubTaskHandle, SubmitGoal},
};

pub struct Factory;

impl Factory {
    pub fn instantiate<S, CF>() -> (
        impl SubmitGoal<S::Goal, Server = S, TaskHandle = PubTaskHandle<S, CF>>,
        Executor,
    )
    where
        S: ServerConcept,
        CF: CancelChannelFactory,
    {
        let (task_sender, task_receiver) = channel(16);

        (
            GoalSubmitter::<S, CF>::new(task_sender),
            Executor::new(task_receiver),
        )
    }
}
