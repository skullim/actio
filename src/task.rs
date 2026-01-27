use tokio::sync::oneshot;

pub struct TaskHandle<R> {
    cancel_sender: oneshot::Sender<()>,
    result_receiver: oneshot::Receiver<R>,
}

impl<R> TaskHandle<R> {
    pub fn new(cancel_sender: oneshot::Sender<()>, result_receiver: oneshot::Receiver<R>) -> Self {
        Self {
            cancel_sender,
            result_receiver,
        }
    }

    pub fn cancel(self) -> oneshot::Receiver<R> {
        // send cancel request irrespectively if other half is still alive
        let _ = self.cancel_sender.send(());
        self.result_receiver
    }

    pub async fn await_response(self) -> R {
        self.result_receiver.await.unwrap()
    }
}
