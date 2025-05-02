use std::pin;

use anyhow::bail;
use futures_util::StreamExt;
use tokio::{
    sync::mpsc,
    time::{self, Sleep},
};

use crate::{
    message,
    server::{self, Server, ServerState, receiver},
};

pub async fn run_root_actor(
    server: &Server,
    receiver: mpsc::Receiver<message::Message>,
    follower_tx: mpsc::Sender<server::ServerRequest>,
    candidate_tx: mpsc::Sender<server::ServerRequest>,
) -> anyhow::Result<()> {
    let mut state = server.state.clone();

    let mut receiver = receiver::ServerReceiver::new(receiver);

    let stream = async_stream::stream! {
        while let Some(item) = receiver.recv().await {
            yield item;
        }
    };
    futures_util::pin_mut!(stream);

    // need to forward messages to the right actor...
    while let Some(message) = stream.next().await {
        println!("GOT: {message:?}");
    }

    Ok(())
}
