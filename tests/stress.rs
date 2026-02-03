mod utils;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use actio::{CancelChannel, Factory, Outcome, SubmitGoal};

    use crate::utils::{STRESS_TEST_TASK_QUEUE_SIZE, await_outcome, poll_executor_for};

    #[tokio::test]
    async fn stress_test() {
        use crate::utils::impls::c::{ProgressGoal, TestServer};

        let mut server = TestServer {};
        let (mut submitter, mut executor) =
            Factory::stateless::<TestServer, CancelChannel>(STRESS_TEST_TASK_QUEUE_SIZE);
        let mut handles: Vec<_> = (0..STRESS_TEST_TASK_QUEUE_SIZE)
            .map(|_| {
                submitter
                    .submit(&mut server, ProgressGoal::default())
                    .unwrap()
            })
            .collect();

        poll_executor_for(Duration::from_millis(42), &mut executor).await;
        let handle = handles.remove(4_200);
        let (outcome_recv, _) = handle.cancel();
        let result = tokio::time::timeout(
            Duration::from_millis(1),
            await_outcome::<TestServer>(outcome_recv, &mut executor),
        )
        .await;
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), Outcome::Cancelled(_)));
    }
}
