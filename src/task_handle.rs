use crate::{
    server::{ServerConcept, ServerOutcome, VisitOutcome, WithFeedback},
    submitting::CancelChannelFactory,
};
use anyhow::anyhow;
use tokio::sync::oneshot;

pub type TaskHandle<S, CF> = GenericTaskHandle<
    tokio::sync::oneshot::Receiver<ServerOutcome<S>>,
    <CF as CancelChannelFactory>::Sender,
    <S as ServerConcept>::FeedbackConfig,
>;

pub struct GenericTaskHandle<R, C, F> {
    result_receiver: R,
    cancel_sender: C,
    feedback_receiver: F,
}

impl<R, C, F> GenericTaskHandle<R, C, F> {
    pub fn new(result_receiver: R, cancel_sender: C, feedback_receiver: F) -> Self {
        Self {
            result_receiver,
            cancel_sender,
            feedback_receiver,
        }
    }

    // pub fn result_receiver(&self) -> &R {
    //     &self.result_receiver
    // }

    // pub fn result_receiver_mut(&mut self) -> &mut R {
    //     &mut self.result_receiver
    // }

    pub fn into_result_receiver(self) -> R {
        self.result_receiver
    }
}
pub struct NoCancel;

// do not expose the sender: cancelling is one-shot action that is guarded by SendCancel trait
pub struct WithCancel<S>(S);

impl<S> WithCancel<S> {
    pub fn new(sender: S) -> Self {
        Self(sender)
    }
}
pub trait CancelConfigMarker {}

impl CancelConfigMarker for NoCancel {}

pub trait CancelSenderMarker {}
pub trait SendCancel {
    fn send(self) -> anyhow::Result<()>;
}

impl<T> CancelSenderMarker for tokio::sync::oneshot::Sender<T> {}

impl SendCancel for tokio::sync::oneshot::Sender<()> {
    fn send(self) -> anyhow::Result<()> {
        //@todo check if error should be handled. Failing means other side dropped, so in theory only on Full there should be meaningful error
        self.send(())
            .map_err(|_| anyhow!("failed to send cancel request"))
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
        (self.result_receiver, self.feedback_receiver)
    }
}

impl<R, C, Rx> GenericTaskHandle<R, C, WithFeedback<Rx>> {
    pub fn feedback_receiver(&self) -> &WithFeedback<Rx> {
        &self.feedback_receiver
    }

    pub fn feedback_receiver_mut(&mut self) -> &mut WithFeedback<Rx> {
        &mut self.feedback_receiver
    }

    pub fn into_parts(self) -> (R, WithFeedback<Rx>) {
        (self.result_receiver, self.feedback_receiver)
    }
}

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

    pub fn into_visitable_result<'a>(
        self,
        server: &'a mut S,
    ) -> VisitableResult<'a, S, ServerOutcome<S>>
    where
        S: VisitOutcome,
    {
        let receiver = self.handle.into_result_receiver();
        VisitableResult { server, receiver }
    }
}

pub struct VisitableResult<'a, S, O> {
    server: &'a mut S,
    receiver: oneshot::Receiver<O>,
}

impl<'a, S> VisitableResult<'a, S, ServerOutcome<S>>
where
    S: VisitOutcome,
{
    pub async fn await_result(self) -> ServerOutcome<S> {
        let outcome = self.receiver.await.unwrap();
        self.server.visit(&outcome);
        outcome
    }
}

impl<S, CF, CS> StatefulTaskHandle<S, CF>
where
    S: ServerConcept,
    CF: CancelChannelFactory<Sender = WithCancel<CS>>,
    CS: SendCancel,
{
    pub async fn cancel<'a>(
        self,
        server: &'a mut S,
    ) -> (VisitableResult<'a, S, ServerOutcome<S>>, S::FeedbackConfig)
    where
        S: VisitOutcome,
    {
        let (result_receiver, feedback_receiver) = self.handle.cancel();
        (
            VisitableResult {
                server,
                receiver: result_receiver,
            },
            feedback_receiver,
        )
    }
}
