pub mod common;

use crate::common::STANDARD_TASK_QUEUE_SIZE;
use actio::{Factory, NoCancelChannel};

#[test]
fn two_servers_impl_same_goal_test() {
    let (_s1, _e1) = Factory::stateless::<crate::common::impls::a::TestServer, NoCancelChannel>(
        STANDARD_TASK_QUEUE_SIZE,
    );
    let (_s2, _e2) = Factory::stateless::<crate::common::impls::b::TestServer, NoCancelChannel>(
        STANDARD_TASK_QUEUE_SIZE,
    );
}
