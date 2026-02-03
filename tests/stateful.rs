#[cfg(test)]
mod utils;

#[cfg(test)]
mod tests {
    use actio::{
        CancelChannel, Factory, NoFeedback, NoTaskStateSnapshot, Outcome, ServerConcept,
        ServerOutcome, ServerSnapshot, ServerTask, SubmitGoal, VisitOutcome,
    };
    use mockall::mock;

    use crate::utils;

    mock! {
        pub VisitOutcome {}

        impl ServerConcept for VisitOutcome {
            type Goal = ();
            type Succeed = ();
            type Failed = ();
            type Feedback = NoFeedback;
            type TaskState = NoTaskStateSnapshot;

            fn create(&mut self, goal: <MockVisitOutcome as ServerConcept>::Goal) -> ServerTask<Self>;
        }

        impl VisitOutcome for VisitOutcome {
            type Error = anyhow::Error;

            fn on_succeed(&mut self, o: &<MockVisitOutcome as ServerConcept>::Succeed) -> Result<(), <MockVisitOutcome as VisitOutcome>::Error>;
            fn on_cancelled(&mut self, o: &ServerSnapshot<Self>) -> Result<(), <MockVisitOutcome as VisitOutcome>::Error>;
            fn on_failed(&mut self, o: &<MockVisitOutcome as ServerConcept>::Failed) -> Result<(), <MockVisitOutcome as VisitOutcome>::Error>;

            fn visit(&mut self, outcome: &ServerOutcome<Self>)-> Result<(), <MockVisitOutcome as VisitOutcome>::Error>;
        }
    }

    #[tokio::test]
    async fn stateful_server_visit_called() {
        let mut mock_server = MockVisitOutcome::new();
        mock_server.expect_create().once().return_once(|()| {
            ServerTask::<MockVisitOutcome>::new(Box::pin(async { Outcome::Succeed(()) }))
        });
        mock_server.expect_visit().times(1).returning(|_| Ok(()));

        let (mut submitter, mut executor) =
            Factory::stateful::<MockVisitOutcome, CancelChannel>(utils::STANDARD_TASK_QUEUE_SIZE);

        let handle = submitter.submit(&mut mock_server, ()).unwrap();
        let visitable_outcome = handle.into_visitable_outcome(&mut mock_server);
        tokio::spawn(async move {
            executor.execute().await;
        });
        let _ = visitable_outcome.outcome().await;
    }
}
