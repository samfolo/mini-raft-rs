use anyhow::bail;
use tokio::{sync::mpsc, task::JoinSet, time};

use crate::{
    naive_logging,
    server::{self, Server, ServerState, request::ServerMessagePayload},
};

/// Each term begins with an election, in which one or more candidates attempt to become leader.
/// If a candidate wins the election, then itserves as leader for the rest of the term.
///
/// To begin an election, a follower increments its current term and transitions to candidate state.
/// It then votes for itself and issues RequestVote RPCs in parallel to each of the other servers
/// in the cluster. A candidate continues in this state until one of three things happens: (a) it
/// wins the election, (b) another server establishes itself as leader, or (c) a period of time
/// goes by with no winner.
pub async fn run_candidate_actor(
    server: &Server,
    mut receiver: mpsc::Receiver<server::Message>,
    cancellation_token: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let mut state = server.state.clone();
    let server_id = server.id;

    let cancelled = cancellation_token.cancelled();
    tokio::pin!(cancelled);

    loop {
        if *state.borrow_and_update() == ServerState::Candidate {
            // Raft uses randomized election timeouts to ensure that split votes are rare and that they are resolved quickly. To
            // prevent split votes in the first place, election timeouts are chosen randomly from a fixed interval.
            // Each candidate restarts its randomized election timeout at the start of an election, and it waits for that timeout
            // to elapse before starting the next election; this reduces the likelihood of another split vote in the new election.
            let timeout = time::sleep(server.generate_random_timeout());
            tokio::pin!(timeout);

            // Increment term
            server.set_current_term(|prev| prev + 1).await;
            let current_term = server.current_term().await;

            // Vote for self
            server.set_voted_for(Some(server_id)).await;

            // Request votes from peers
            let mut join_set = JoinSet::new();
            for (_, peer_handle) in server.peer_list.peers_iter() {
                let peer_handle = peer_handle.clone();

                join_set.spawn(async move {
                    peer_handle
                        .request_vote(server_id, current_term)
                        .await
                        .map_err(|err| anyhow::anyhow!("failed to request vote: {err:?}"))
                });
            }

            let acks = join_set.join_all().await;

            // Tally received votes over term
            let total_votes_requested = acks.len();
            let mut total_votes_over_term = 0;

            'election: loop {
                tokio::select! {
                    // If many followers become candidates at the same time, votes could be split so that no candidate obtains a
                    // majority. When this happens, each candidate will time out and start a new election by incrementing its
                    // term and initiating another round of RequestVote RPCs.
                    _ = &mut timeout => {
                        naive_logging::log(&server.id, "failed to receive enough votes this term.");
                        naive_logging::log(&server.id, "restarting election...");
                        break 'election;
                    }
                    res = state.changed() => {
                        if let Err(err) = res {
                            bail!("{:?}", err);
                        }
                        break 'election;
                    }
                    _ = &mut cancelled => {
                        naive_logging::log(&server.id, "shutting down candidate routine...");
                        return Ok(())
                    }
                    Some(msg) = receiver.recv() => {
                        match msg {
                            server::Message::Request(req) => {
                                let request_term = req.term();

                                match req.body() {
                                    // While waiting for votes, a candidate may receive an AppendEntries RPC from another server
                                    // claiming to be leader.

                                    server::ServerRequestBody::AppendEntries {
                                        leader_id,
                                        prev_log_index,
                                        prev_log_term,
                                        entries,
                                        leader_commit,
                                    } => {
                                        naive_logging::log(
                                            &server.id,
                                            &format!("<- APPEND_ENTRIES (req) {{ \
                                                term: {request_term}, \
                                                leader_id: {leader_id}, \
                                                entries: {entries:?} \
                                            }}"),
                                        );

                                        let sender_handle = server.peer_list.get(&req.sender_id()).unwrap();

                                        // If the leader’s term (included in its RPC) is at least as large as
                                        // the candidate’s current term, then the candidate recognizes the leader as legitimate and
                                        // returns to follower state. If the term in the RPC is smaller than the candidate’s
                                        // current term, then the candidate rejects the RPC and continues in candidate state.
                                        let success = request_term >= current_term;
                                        if success {
                                            naive_logging::log(
                                                &server.id,
                                                &format!("acknowledging new leader... {{ \
                                                    current_term: {current_term}, \
                                                    request_term: {request_term}, \
                                                    leader_id: {leader_id} \
                                                }}"),
                                            );

                                            server.set_current_term(|_| request_term).await;
                                            server.set_voted_for(Some(*leader_id)).await;
                                            if let Err(err) = server.downgrade_to_follower() {
                                                bail!("failed to downgrade to follower: {err:?}");
                                            }
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
                                            &format!("<- REQUEST_VOTE (req) {{ \
                                                term: {request_term}, \
                                                candidate_id: {candidate_id} \
                                            }}"),
                                        );

                                        let sender_handle = server.peer_list.get(&req.sender_id()).unwrap();

                                        let vote_granted = request_term >= current_term;
                                        if vote_granted {
                                            naive_logging::log(
                                                &server.id,
                                                &format!("backing out of election... {{ \
                                                    current_term: {current_term}, \
                                                    request_term: {request_term}, \
                                                    candidate_id: {candidate_id} \
                                                }}"),
                                            );

                                            server.set_current_term(|_| request_term).await;
                                            server.set_voted_for(Some(*candidate_id)).await;
                                            if let Err(err) = server.downgrade_to_follower() {
                                                bail!("failed to downgrade to follower: {err:?}");
                                            }
                                            sender_handle.request_vote_response(server_id, current_term, true).await?;
                                        } else {
                                            naive_logging::log(
                                                &server.id,
                                                &format!("ignoring opposing candidate... {{ \
                                                    current_term: {current_term}, \
                                                    request_term: {request_term}, \
                                                    candidate_id: {candidate_id} \
                                                }}"),
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
                                    },
                                    server::ServerResponseBody::RequestVote { vote_granted } => {
                                        naive_logging::log(
                                            &server.id,
                                            &format!("<- REQUEST_VOTE (res) {{ \
                                                term: {response_term}, \
                                                vote_granted: {vote_granted} \
                                            }}"),
                                        );

                                        if *vote_granted {
                                            total_votes_over_term += 1;

                                            // A candidate wins an election if it receives votes from a majority of the servers
                                            // in the full cluster for the same term. Each server will vote for at most one candidate
                                            // in a given term, on a first-come-first-served basis.
                                            //
                                            // The majority rule ensures that at most one candidate can win the election for a
                                            // particular term.
                                            if total_votes_over_term * 2 > total_votes_requested {
                                                naive_logging::log(&server.id, "received a majority of votes this term.");
                                                // Once a candidate wins an election, it becomes leader. It then sends heartbeat
                                                // messages to all of the other servers to establish its authority and prevent new
                                                // elections.
                                                if let Err(err) = server.upgrade_to_leader() {
                                                    bail!("failed to upgrade to leader: {err:?}");
                                                }
                                                break 'election;
                                            }
                                        }
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
                naive_logging::log(&server.id, "shutting down candidate routine...");
                return Ok(())
              }
            }
        }
    }
}
