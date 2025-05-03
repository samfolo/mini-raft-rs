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
                naive_logging::log(&server.id, "shutting down client request handler...");
                return Ok(())
            }
            Some(msg) = receiver.recv() => {
                match msg {
                    client::Message::Request(req) => {
                        naive_logging::log(&server.id, &match req.body {
                            client::ClientRequestBody::Read => format!("<- CLIENT_READ_CMD (req) {{ }}"),
                            client::ClientRequestBody::Write { command } => format!("<- CLIENT_WRITE_CMD (req) {{ command: {command} }}")
                        });

                        if *state.borrow_and_update() == ServerState::Leader {
                            match req.body {
                                client::ClientRequestBody::Read => {
                                    req.responder.handle_client_read_response(server.id, server.state_machine.get_snapshot()).await?;
                                },
                                client::ClientRequestBody::Write { command } => {
                                    server.append_to_log(command).await;
                                    req.responder.handle_client_write_response(server.id, true).await?;
                                }
                            }
                        } else {
                            let leader_id = server.voted_for().await.unwrap();
                            naive_logging::log(&server.id, &format!(">> forwarding to current leader... {{ leader_id: {leader_id} }}"));

                            let leader_handle = server.peer_list.get(&leader_id).unwrap();
                            leader_handle.handle_client_request(&server.id, req, true).await?;
                        }
                    }
                    client::Message::Response(_) => unreachable!("should never have received this message"),
                }
            }
        }
    }
}
