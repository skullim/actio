#[cfg(test)]
mod utils;

#[cfg(test)]
mod tests {
    use crate::utils::STANDARD_TASK_QUEUE_SIZE;
    use actio::{Factory, NoCancelChannel};

    #[test]
    fn two_servers_impl_same_goal_test() {
        let (_s1, _e1) = Factory::stateless::<crate::utils::impls::a::TestServer, NoCancelChannel>(
            STANDARD_TASK_QUEUE_SIZE,
        );
        let (_s2, _e2) = Factory::stateless::<crate::utils::impls::b::TestServer, NoCancelChannel>(
            STANDARD_TASK_QUEUE_SIZE,
        );
    }
}
