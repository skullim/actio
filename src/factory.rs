use futures::channel::mpsc::channel;

use crate::{
    execution::Executor,
    server::ServerConcept,
    submitting::{CancelChannelFactory, GoalSubmitter, StatefulGoalSubmitter, SubmitGoal},
    task_handle::{StatefulTaskHandle, TaskHandle},
};

/// [`Factory`] provides API to construct a pair:
/// - a goal-submission interface, and
/// - an [`Executor`] that must be driven to run submitted tasks.
///   Each [`Executor`] is bound to the particular  [`ServerConcept`] **type**
pub struct Factory;

impl Factory {
    /// Creates stateless submission interface for a [`ServerConcept`].
    /// Recommended when the server does not need to update internal state based on the terminal [`crate::Outcome`].    
    #[must_use]
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
    /// Creates stateful submission interface for a [`ServerConcept`].
    /// Use this when the server must update internal state based on the terminal [`crate::Outcome`] (via [`crate::VisitOutcome`]).
    /// Returned goal submitter returns [`crate::StatefulTaskHandle`] that requires server instance to update its state based on the terminal [`crate::Outcome`].  
    #[must_use]
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
