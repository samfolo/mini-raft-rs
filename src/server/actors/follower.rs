use std::pin;

use anyhow::bail;
use tokio::{
    sync::mpsc,
    time::{self, Sleep},
};

use crate::{
    naive_logging,
    server::{self, Server, ServerState, request::ServerMessagePayload},
};

/// Followers are passive: they issue no requests on their own but simply
/// respond to requests from leaders and candidates.
pub async fn run_follower_actor(
    server: &Server,
    mut receiver: mpsc::Receiver<server::Message>,
    cancellation_token: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let mut state = server.state.clone();

    let cancelled = cancellation_token.cancelled();
    tokio::pin!(cancelled);

    let timeout_dur = server.generate_random_timeout();
    let timeout = time::sleep(timeout_dur);
    tokio::pin!(timeout);

    let reset_timeout = |timeout: &mut pin::Pin<&mut Sleep>| {
        timeout.set(time::sleep(server.generate_random_timeout()))
    };

    loop {
        if *state.borrow_and_update() == ServerState::Follower {
            // Wait for a message, or a random timeout
            // If message, reset the random timeout
            // else upgrade to candidate
            tokio::select! {
              _ = &mut timeout => {
                if let Err(err) = server.upgrade_to_candidate() {
                  bail!("failed to upgrade to candidate: {err:?}");
                }
              }
              res = state.changed() => {
                if let Err(err) = res {
                  bail!("failed to read server state: {err:?}");
                }
              }
              _ = &mut cancelled => {
                return Ok(())
              }
              Some(msg) = receiver.recv() => {
                match msg {
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
                }

                // decide when this happens:
                reset_timeout(&mut timeout);
              }
            }
        } else {
            tokio::select! {
              res = state.changed() => {
                if let Err(err) = res {
                  bail!("{:?}", err);
                }

                reset_timeout(&mut timeout);
              }
              _ = &mut cancelled => {
                return Ok(())
              }
            }
        }
    }
}
