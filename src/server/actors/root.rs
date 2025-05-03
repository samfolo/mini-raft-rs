use std::pin;

use anyhow::bail;
use futures_util::StreamExt;
use tokio::{
    sync::mpsc,
    time::{self, Sleep},
};

use crate::{
    message, naive_logging,
    server::{self, Server, ServerState, receiver, request::ServerMessagePayload},
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
        match message {
            message::Message::Server(server_msg) => match server_msg {
                server::Message::Request(req) => {
                    let request_term = req.term();

                    match req.body() {
                        server::ServerRequestBody::AppendEntries { leader_id, entries } => {
                            naive_logging::log(
                                &server.id,
                                &format!(
                                    "<- APPEND_ENTRIES (req) {{ term: {request_term}, leader_id: {leader_id}, entries: {entries:?} }}"
                                ),
                            );
                        }
                        server::ServerRequestBody::RequestVote { candidate_id } => {
                            naive_logging::log(
                                &server.id,
                                &format!(
                                    "<- REQUEST_VOTE (req) {{ term: {request_term}, candidate_id: {candidate_id} }}"
                                ),
                            );
                        }
                    }
                }
                server::Message::Response(res) => match res.body() {
                    server::ServerResponseBody::AppendEntries {} => {
                        naive_logging::log(&server.id, "<- APPEND_ENTRIES (res) { }");
                    }
                    server::ServerResponseBody::RequestVote { vote_granted } => {
                        naive_logging::log(
                            &server.id,
                            &format!("<- REQUEST_VOTE (res) {{ vote_granted: {vote_granted} }}"),
                        );
                    }
                },
            },
            m => println!("CLIENT: {m:?}"),
        }
    }

    Ok(())
}
