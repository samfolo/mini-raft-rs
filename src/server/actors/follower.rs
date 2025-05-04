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
    let server_id = server.id;

    let cancelled = cancellation_token.cancelled();
    tokio::pin!(cancelled);

    let timeout = time::sleep(server.generate_random_timeout());
    tokio::pin!(timeout);

    let reset_timeout = |timeout: &mut pin::Pin<&mut Sleep>| {
        timeout.set(time::sleep(server.generate_random_timeout()))
    };

    loop {
        if *state.borrow_and_update() == ServerState::Follower {
            let current_term = server.current_term().await;

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
                    naive_logging::log(&server.id, "shutting down follower routine...");
                    return Ok(())
                }
                Some(msg) = receiver.recv() => {
                    match msg {
                        server::Message::Request(req) => {
                            let request_term = req.term();

                            match req.body() {
                                server::ServerRequestBody::AppendEntries {
                                    leader_id,
                                    prev_log_index,
                                    prev_log_term,
                                    entries,
                                    leader_commit,
                                } => {
                                    naive_logging::log(
                                        &server.id,
                                        &format!(
                                            "<- APPEND_ENTRIES (req) {{ \
                                                term: {request_term}, \
                                                leader_id: {leader_id}, \
                                                entries: {entries:?} \
                                            }}"
                                        ),
                                    );

                                    let sender_handle = server.peer_list.get(&req.sender_id()).unwrap();

                                    let success = request_term >= current_term;
                                    if success {
                                        if server.voted_for().await.is_none_or(|id| id != *leader_id) {
                                            naive_logging::log(
                                                &server.id,
                                                &format!("acknowledging new leader... {{ \
                                                    current_term: {current_term}, \
                                                    request_term: {request_term}, \
                                                    leader_id: {leader_id} \
                                                }}"),
                                            );
                                        }
                                        server.set_voted_for(Some(*leader_id)).await;

                                        server.set_current_term(|_| request_term).await;
                                        sender_handle.append_entries_response(server_id, current_term, true).await?;
                                    } else {
                                        naive_logging::log(
                                            &server.id,
                                            &format!("ignoring request from stale leader... {{ \
                                                current_term: {current_term}, \
                                                request_term: {request_term}, \
                                                leader_id: {leader_id} \
                                            }}"),
                                        );

                                        sender_handle.append_entries_response(server_id, current_term, false).await?;
                                    }
                                }
                                server::ServerRequestBody::RequestVote { candidate_id } => {
                                    naive_logging::log(
                                        &server.id,
                                        &format!(
                                            "<- REQUEST_VOTE (req) {{ \
                                                term: {request_term}, \
                                                candidate_id: {candidate_id} \
                                            }}"
                                        ),
                                    );

                                    let sender_handle = server.peer_list.get(&req.sender_id()).unwrap();

                                    let vote_granted = request_term >= current_term && server.voted_for().await.is_none_or(|id| id == *candidate_id);
                                    if vote_granted {
                                        server.set_current_term(|_| request_term).await;
                                        server.set_voted_for(Some(*candidate_id)).await;
                                        if let Err(err) = server.downgrade_to_follower() {
                                            bail!("failed to downgrade to follower: {err:?}");
                                        }
                                        sender_handle.request_vote_response(server_id, current_term, true).await?;
                                    } else {
                                        naive_logging::log(
                                            &server.id,
                                            &format!(
                                                "refusing vote for candidate... {{ \
                                                    current_term: {current_term}, \
                                                    request_term: {request_term}, \
                                                    voted_for: {:?} \
                                                }}",
                                                match server.voted_for().await {
                                                    Some(id) => format!("Some({id})"),
                                                    None => "None".to_string()
                                                },
                                            ),
                                        );

                                        sender_handle.request_vote_response(server_id, current_term, false).await?;
                                    }
                                }
                            }
                        }
                        server::Message::Response(res) => {
                            let response_term = res.term();

                            match res.body() {
                                server::ServerResponseBody::AppendEntries { success } => {
                                    naive_logging::log(&server.id, &format!("<- APPEND_ENTRIES (res) {{ \
                                        term: {response_term}, \
                                        success: {success} \
                                    }}"));
                                    unreachable!("should never have received this message");
                                }
                                server::ServerResponseBody::RequestVote { vote_granted } => {
                                    naive_logging::log(
                                        &server.id,
                                        &format!("<- REQUEST_VOTE (res) {{ \
                                            term: {response_term}, \
                                            vote_granted: {vote_granted} \
                                        }}"),
                                    );
                                }
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
                    naive_logging::log(&server.id, "shutting down follower routine...");
                    return Ok(())
                }
            }
        }
    }
}
