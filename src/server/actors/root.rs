use futures_util::StreamExt;
use tokio::sync::mpsc;

use crate::{
    client, message,
    server::{self, Server, ServerState, receiver},
};

pub async fn run_root_actor(
    server: &Server,
    receiver: mpsc::Receiver<message::Message>,
    follower_tx: mpsc::Sender<server::Message>,
    candidate_tx: mpsc::Sender<server::Message>,
    leader_tx: mpsc::Sender<server::Message>,
    client_request_tx: mpsc::Sender<client::Message>,
) -> anyhow::Result<()> {
    let mut state = server.state.clone();

    let mut receiver = receiver::ServerReceiver::new(receiver);

    let stream = async_stream::stream! {
        while let Some(item) = receiver.recv().await {
            yield item;
        }
    };
    futures_util::pin_mut!(stream);

    while let Some(message) = stream.next().await {
        match message {
            message::Message::Server(server_msg) => match *state.borrow_and_update() {
                ServerState::Follower => follower_tx.send(server_msg).await?,
                ServerState::Candidate => candidate_tx.send(server_msg).await?,
                ServerState::Leader => leader_tx.send(server_msg).await?,
            },
            message::Message::Client(client_msg) => client_request_tx.send(client_msg).await?,
        }
    }

    Ok(())
}
