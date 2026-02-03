mod utils;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::utils::impls::Goal;
    use crate::utils::{STANDARD_TASK_QUEUE_SIZE, await_outcome, poll_executor_for};
    use actio::{CancelChannel, Factory, Outcome, SubmitGoal};

    #[tokio::test]
    async fn no_request_cancel_test() {
        use crate::utils::impls::a::TestServer;

        let mut server = TestServer {};
        let (mut submitter, mut executor) =
            Factory::stateless::<TestServer, CancelChannel>(STANDARD_TASK_QUEUE_SIZE);
        let handle = submitter.submit(&mut server, Goal::default()).unwrap();
        let outcome_recv = handle.into_outcome_receiver();

        let out = await_outcome::<TestServer>(outcome_recv, &mut executor).await;
        assert!(matches!(out, Outcome::Succeed(_)));
    }

    #[tokio::test]
    async fn request_cancel_test() {
        use crate::utils::impls::a::TestServer;

        let mut server = TestServer {};
        let (mut submitter, mut executor) =
            Factory::stateless::<TestServer, CancelChannel>(STANDARD_TASK_QUEUE_SIZE);
        let handle = submitter.submit(&mut server, Goal::default()).unwrap();
        let (outcome_recv, _) = handle.cancel();

        let out = await_outcome::<TestServer>(outcome_recv, &mut executor).await;
        assert!(matches!(out, Outcome::Cancelled(_)));
    }

    #[tokio::test]
    async fn cancel_with_exe_ctx_test() {
        use crate::utils::impls::c::{ProgressGoal, TaskState, TestServer};

        let mut server = TestServer {};
        let (mut submitter, mut executor) =
            Factory::stateless::<TestServer, CancelChannel>(STANDARD_TASK_QUEUE_SIZE);

        let handle = submitter
            .submit(&mut server, ProgressGoal::default())
            .unwrap();
        let (outcome_recv, _) = handle.cancel();
        let out = await_outcome::<TestServer>(outcome_recv, &mut executor).await;
        assert!(matches!(out, Outcome::Cancelled(TaskState { progress: 0 })));

        let handle = submitter
            .submit(&mut server, ProgressGoal::default())
            .unwrap();
        poll_executor_for(Duration::from_millis(100), &mut executor).await;

        let (outcome_recv, _) = handle.cancel();

        let out = await_outcome::<TestServer>(outcome_recv, &mut executor).await;
        assert!(matches!(
            out,
            Outcome::Cancelled(TaskState { progress: 50 })
        ));
    }
}
