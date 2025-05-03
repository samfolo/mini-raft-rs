use tokio::sync::mpsc;

use crate::client;

pub struct ClientReceiver {
    rx: mpsc::Receiver<client::Message>,
}

impl ClientReceiver {
    pub fn new(rx: mpsc::Receiver<client::Message>) -> Self {
        Self { rx }
    }

    pub fn recv(&mut self) -> impl Future<Output = Option<client::Message>> {
        self.rx.recv()
    }
}
