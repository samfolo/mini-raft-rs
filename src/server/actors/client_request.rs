use anyhow::bail;
use tokio::sync::mpsc;

use crate::{
    client, naive_logging,
    server::{Server, ServerState},
};

/// The leader handles all client requests (if a client contacts a follower,
/// the follower redirects it to the leader).
pub async fn run_client_request_actor(
    server: &Server,
    mut receiver: mpsc::Receiver<client::Message>,
    cancellation_token: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let mut state = server.state.clone();

    let cancelled = cancellation_token.cancelled();
    tokio::pin!(cancelled);

    loop {
        tokio::select! {
            res = state.changed() => {
                if let Err(err) = res {
                    bail!("{:?}", err);
                }
            }
            _ = &mut cancelled => {
                return Ok(())
            }
            Some(msg) = receiver.recv() => {
                match msg {
                    client::Message::Request(req) => {
                        naive_logging::log(&server.id, &format!("<- CLIENT_REQUEST {{ body: {} }}", req.body()));
                        if *state.borrow_and_update() == ServerState::Leader {
                            naive_logging::log(&server.id, &format!("handling..."));

                            todo!("handle client requests");
                        } else {
                            let leader_id = server.voted_for().unwrap();
                            naive_logging::log(&server.id, &format!("forwarding to current leader... {{ leader_id: {leader_id} }}"));

                            let leader_handle = server.peer_list.get(&leader_id).unwrap();
                            leader_handle.handle_client_request(&server.id, *req.body()).await?;
                        }
                    }
                    client::Message::Response(_) => unreachable!(),
                }
            }
        }
    }
}
