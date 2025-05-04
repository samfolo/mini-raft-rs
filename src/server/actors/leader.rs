use anyhow::bail;
use tokio::{sync::mpsc, task::JoinSet, time};

use crate::{
    naive_logging,
    server::{self, Server, ServerState, request::ServerMessagePayload},
};

/// Once a leader has been elected, it begins servicing client requests. Each client request contains a command
/// to be executed by the replicated state machines. The leader appends the command to its log as a new entry, then
/// issues AppendEntries RPCs in parallel to each of the other servers to replicate the entry.
pub async fn run_leader_actor(
    server: &Server,
    mut receiver: mpsc::Receiver<server::Message>,
    cancellation_token: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let mut state = server.state.clone();
    let server_id = server.id;

    let cancelled = cancellation_token.cancelled();
    tokio::pin!(cancelled);

    loop {
        if *state.borrow_and_update() == ServerState::Leader {
            let current_term = server.current_term().await;

            // timeout controls
            let timeout = time::sleep(server.heartbeat_interval);
            tokio::pin!(timeout);

            let leader_commit = server.commit_index().await;

            // append entries to followers
            let mut join_set = JoinSet::new();
            for (peer_id, peer_handle) in server.peer_list.peers_iter() {
                let peer_handle = peer_handle.clone();

                let peer_next_index = match server.get_next_index_for_peer(&peer_id).await {
                    Some(index) => index,
                    None => bail!("failed to get next index for peer with ID {peer_id}"),
                };

                let prev_log_index = 0;
                let prev_log_term = 0;

                join_set.spawn(async move {
                    peer_handle
                        .append_entries(
                            server_id,
                            prev_log_index,
                            prev_log_term,
                            vec![],
                            leader_commit,
                            current_term,
                        )
                        .await
                        .map_err(|err| anyhow::anyhow!("failed to append entries: {err:?}"))
                });
            }

            let _ = join_set.join_all().await;

            'heartbeat: loop {
                tokio::select! {
                    _ = &mut timeout => {
                        break 'heartbeat;
                    }
                    res = state.changed() => {
                        if let Err(err) = res {
                            bail!("{:?}", err);
                        }
                        break 'heartbeat;
                    }
                    _ = &mut cancelled => {
                        naive_logging::log(&server.id, "shutting down leader routine...");
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

                                        todo!("fencing scenario");
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

                                        todo!("fencing scenario");
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
                                        if *success {
                                            // track the commit tally vector here
                                        }
                                    },
                                    server::ServerResponseBody::RequestVote { vote_granted } => {
                                        naive_logging::log(&server.id, &format!("<- REQUEST_VOTE (res) {{ \
                                            term: {response_term}, \
                                            vote_granted: {vote_granted} \
                                        }}"));
                                        naive_logging::log(&server.id, "no longer campaigning; ignoring vote...");
                                    }
                                }
                            },
                        }
                    }
                }
            }
        } else {
            tokio::select! {
              res = state.changed() => {
                if let Err(err) = res {
                  bail!("{:?}", err);
                }
              }
              _ = &mut cancelled => {
                naive_logging::log(&server.id, "shutting down leader routine...");
                return Ok(())
              }
            }
        }
    }
}
