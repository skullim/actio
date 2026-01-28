pub enum Outcome<S, C, F> {
    Succeed(S),
    Cancelled(C),
    Failed(F),
}

pub trait ServerConcept<G> {
    type Task;

    fn create(&mut self, goal: G) -> Self::Task;
}
