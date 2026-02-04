use crate::{
    Error, Result,
    server::{ServerConcept, ServerOutcome, VisitOutcome, WithFeedback},
    submitting::CancelChannelFactory,
};
use futures::channel::oneshot;
use std::result::Result as StdResult;

/// Handle for a server task wired to a particular channel factory (CF)  and  [`ServerConcept`] implementation,
pub type TaskHandle<S, CF> = GenericTaskHandle<
    oneshot::Receiver<ServerOutcome<S>>,
    <CF as CancelChannelFactory>::Sender,
    <S as ServerConcept>::Feedback,
>;

pub struct GenericTaskHandle<R, C, F> {
    outcome_receiver: R,
    cancel_sender: C,
    feedback_receiver: F,
}

impl<R, C, F> GenericTaskHandle<R, C, F> {
    pub fn new(outcome_receiver: R, cancel_sender: C, feedback_receiver: F) -> Self {
        Self {
            outcome_receiver,
            cancel_sender,
            feedback_receiver,
        }
    }

    pub fn into_outcome_receiver(self) -> R {
        self.outcome_receiver
    }
}
pub struct NoCancel;

/// Cancellation enabled wrapper around a cancel-sender (capability-guarded).
pub struct WithCancel<S>(S);
impl<S> WithCancel<S> {
    pub(crate) fn new(sender: S) -> Self {
        Self(sender)
    }
}
pub trait CancelConfigMarker {}

impl CancelConfigMarker for NoCancel {}

pub trait CancelSenderMarker {}
pub trait SendCancel {
    fn send(self) -> Result<()>;
}

impl<T> CancelSenderMarker for oneshot::Sender<T> {}

impl SendCancel for oneshot::Sender<()> {
    fn send(self) -> Result<()> {
        self.send(()).map_err(|()| Error::CancelSendFailure)
    }
}

impl<S> CancelConfigMarker for WithCancel<S> where S: CancelSenderMarker + SendCancel {}

impl<R, S, F> GenericTaskHandle<R, WithCancel<S>, F>
where
    S: SendCancel,
{
    pub fn cancel(self) -> (R, F) {
        // can fail only if channel full, but since we sent and consume it should never happen
        let _ = self.cancel_sender.0.send();
        (self.outcome_receiver, self.feedback_receiver)
    }
}

impl<R, C, Rx> GenericTaskHandle<R, C, WithFeedback<Rx>> {
    pub fn feedback_receiver_mut(&mut self) -> &mut WithFeedback<Rx> {
        &mut self.feedback_receiver
    }

    pub fn into_parts(self) -> (R, WithFeedback<Rx>) {
        (self.outcome_receiver, self.feedback_receiver)
    }
}

/// Stateful task handle: lets the server visit the terminal outcome before returning it.
pub struct StatefulTaskHandle<S, CF>
where
    S: ServerConcept,
    CF: CancelChannelFactory,
{
    handle: TaskHandle<S, CF>,
}

impl<S, CF> StatefulTaskHandle<S, CF>
where
    S: ServerConcept,
    CF: CancelChannelFactory,
{
    pub fn new(handle: TaskHandle<S, CF>) -> Self {
        Self { handle }
    }

    pub fn into_visitable_outcome(self, server: &mut S) -> VisitableOutcome<'_, S, ServerOutcome<S>>
    where
        S: VisitOutcome,
    {
        let receiver = self.handle.into_outcome_receiver();
        VisitableOutcome { server, receiver }
    }
}

/// Future-like wrapper that awaits the outcome and then calls [`VisitOutcome::visit`].
pub struct VisitableOutcome<'a, S, O> {
    server: &'a mut S,
    receiver: oneshot::Receiver<O>,
}

impl<S> VisitableOutcome<'_, S, ServerOutcome<S>>
where
    S: VisitOutcome,
{
    pub async fn outcome(self) -> StdResult<ServerOutcome<S>, S::Error> {
        #[expect(clippy::missing_panics_doc, reason = "infallible")]
        // awaiting receiver can only fail if sender part is dropped. Since sender is always part of
        // executed task this can never happen
        let outcome = self.receiver.await.unwrap();
        self.server.visit(&outcome)?;
        Ok(outcome)
    }
}

impl<S, CF, CS> StatefulTaskHandle<S, CF>
where
    S: ServerConcept,
    CF: CancelChannelFactory<Sender = WithCancel<CS>>,
    CS: SendCancel,
{
    pub fn cancel(self, server: &mut S) -> (VisitableOutcome<'_, S, ServerOutcome<S>>, S::Feedback)
    where
        S: VisitOutcome,
    {
        let (outcome_receiver, feedback_receiver) = self.handle.cancel();
        (
            VisitableOutcome {
                server,
                receiver: outcome_receiver,
            },
            feedback_receiver,
        )
    }
}
