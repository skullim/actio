use tokio::sync::{mpsc, oneshot};

pub struct SimpleTask<F> {
    pub task: F,
}

pub struct TaskWithFeedback<F, Fb> {
    pub task: F,
    pub feedback_rx: mpsc::Receiver<Fb>,
}

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

pub struct TaskHandleWithFeedback<R, Fb> {
    pub cancel: oneshot::Sender<()>,
    pub result: oneshot::Receiver<R>,
    pub feedback: mpsc::Receiver<Fb>,
}

impl<R, Fb> TaskHandleWithFeedback<R, Fb> {
    pub fn new(
        cancel: oneshot::Sender<()>,
        result: oneshot::Receiver<R>,
        feedback: mpsc::Receiver<Fb>,
    ) -> Self {
        Self {
            cancel,
            result,
            feedback,
        }
    }
}
