use tokio::sync::mpsc;

use crate::message;

pub struct ServerReceiver {
    rx: mpsc::Receiver<message::Message>,
}

impl ServerReceiver {
    pub fn new(rx: mpsc::Receiver<message::Message>) -> Self {
        Self { rx }
    }

    pub fn recv(&mut self) -> impl Future<Output = Option<message::Message>> {
        self.rx.recv()
    }
}
