use futures_util::{Stream, StreamExt};
use tokio::sync::mpsc;

use crate::message;

pub struct ServerReceiver {
    recv: mpsc::Receiver<message::Message>,
}

impl ServerReceiver {
    pub fn new(recv: mpsc::Receiver<message::Message>) -> Self {
        Self { recv }
    }

    pub async fn stream_recv(mut self) -> impl Stream {
        async_stream::stream! {
            while let Some(item) = self.recv.recv().await {
                yield item;
            }
        }
    }
}
