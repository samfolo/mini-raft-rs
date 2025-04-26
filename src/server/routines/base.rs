use std::pin;
use tokio::time;

use crate::{cluster_node, naive_logging, server};
use cluster_node::error::ClusterNodeError;
use server::{Server, ServerState, rpc};

impl Server {
    pub(in crate::server) async fn run_base_routine(&self) -> cluster_node::Result<()> {
        let state = self.state_tx.subscribe();
        let mut subscriber = self.publisher.subscribe();

        let timeout = time::sleep(self.election_timeout_range.random());
        tokio::pin!(timeout);

        let reset_timeout = |timeout: &mut pin::Pin<&mut time::Sleep>| {
            timeout
                .as_mut()
                .reset(time::Instant::now() + self.election_timeout_range.random())
        };

        loop {
            tokio::select! {
                _ = &mut timeout => {
                    if *state.borrow() == ServerState::Follower {
                        naive_logging::log(&self.id, "timed out waiting for a response...");
                        naive_logging::log(&self.id, "starting election...");

                        self.vote(Some(self.id));

                        if let Err(err) = self.upgrade_to_candidate() {
                            return Err(ClusterNodeError::Unexpected(err.into()));
                        }
                    }
                },
                res = subscriber.recv() => {
                    match res {
                        Ok(request) => {
                            match request.body() {
                                rpc::RequestBody::AppendEntries { leader_id, entries } => {
                                    if self.id != *leader_id {
                                        naive_logging::log(
                                            &self.id,
                                            &format!(
                                                "{} <- APPEND_ENTRIES {{ term: {}, leader_id: {}, entries: {:?} }}",
                                                self.listener, request.term(), leader_id, entries
                                            ),
                                        );

                                        let current_term =  self.current_term();

                                        let is_stale_request = request.term() < current_term;

                                        if is_stale_request {
                                            naive_logging::log(&self.id, &format!("rejecting request from stale leader {leader_id}"));
                                        } else {
                                            // Received request from leader; reset the election timeout:
                                            reset_timeout(&mut timeout);

                                            if self.voted_for().is_none_or(|id| id != *leader_id) {
                                                naive_logging::log(&self.id, &format!("acknowledging new leader {leader_id}"));
                                                self.vote(Some(*leader_id));
                                            }
                                        }

                                        if request.can_respond() {
                                            if let Err(err) = request.respond(
                                                current_term,
                                                rpc::ResponseBody::AppendEntries { },
                                            ).await {
                                                return Err(ClusterNodeError::Unexpected(err.into()));
                                            }
                                        }

                                        if let Err(err) = self.sync_term(request.term()).await {
                                            return Err(ClusterNodeError::Unexpected(err.into()));
                                        }
                                    }
                                }
                                rpc::RequestBody::RequestVote { candidate_id, .. } => {
                                    if self.id != *candidate_id {
                                        // Received request from candidate:
                                        naive_logging::log(
                                            &self.id,
                                            &format!(
                                                "{} <- REQUEST_VOTE {{ term: {}, candidate_id: {} }}",
                                                self.listener,
                                                request.term(),
                                                candidate_id,
                                            ),
                                        );

                                        let current_term = self.current_term();

                                        let vote_granted = request.term() >= current_term;

                                        if vote_granted {
                                            reset_timeout(&mut timeout);
                                            naive_logging::log(&self.id, &format!("granting vote to candidate {candidate_id}"));
                                            self.vote(Some(*candidate_id));
                                        } else {
                                            naive_logging::log(&self.id, &format!("refusing vote for candidate {candidate_id}: term={current_term}, req_term={}", request.term()));
                                        }

                                        if request.can_respond() {
                                            if let Err(err) = request.respond(
                                                current_term,
                                                rpc::ResponseBody::RequestVote { vote_granted },
                                            ).await {
                                                return Err(ClusterNodeError::Unexpected(err.into()));
                                            }
                                        }

                                        if let Err(err) = self.sync_term(request.term()).await {
                                            return Err(ClusterNodeError::Unexpected(err.into()));
                                        }
                                    }
                                }
                            }
                        },
                        // Tracing would be nice here..
                        Err(err) => return Err(ClusterNodeError::IncomingClusterConnection(self.id, err))
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {}
