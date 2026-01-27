//@todo add a way to easily integrate feedback
pub trait ActionBehavior {
    type Goal;
    type Result: 'static;

    /// implementation must guarantee that the future is safe to cancel, or more concretely it must exhibit cancel correctness
    fn create(&mut self, goal: Self::Goal) -> impl Future<Output = Self::Result> + Send + 'static;

    /// API to update state based on the result
    fn visit_result(&mut self, _r: &Self::Result) {}
}

pub enum Outcome<S, C, F> {
    Succeed(S),
    Cancelled(C),
    Failed(F),
}
