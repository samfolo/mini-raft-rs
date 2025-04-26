use tokio::sync::mpsc;
use tokio::time;

use crate::{cluster_node, naive_logging, server};
use cluster_node::error::ClusterNodeError;
use server::{Server, ServerState, rpc};

impl Server {
    pub(in crate::server) async fn run_election_routine(&self) -> cluster_node::Result<()> {
        let mut state = self.state_tx.subscribe();

        loop {
            if *state.borrow() == ServerState::Candidate {
                'election_loop: loop {
                    self.set_current_term(|prev| prev + 1);

                    match self.request_vote() {
                        Ok(mut response) => {
                            let timeout = self.election_timeout_range.random();

                            if let Err(err) = self
                                .handle_timebound_request_vote_response(timeout, &mut response)
                                .await
                            {
                                match err {
                                    ClusterNodeError::Timeout(_) => {
                                        naive_logging::log(
                                            &self.id,
                                            "election ended before enough votes were received.",
                                        );
                                        naive_logging::log(&self.id, "restarting election...");
                                    }
                                    other_err => return Err(other_err),
                                }
                            } else {
                                break 'election_loop;
                            }
                        }
                        Err(err) => {
                            return Err(ClusterNodeError::OutgoingClusterConnection(self.id, err));
                        }
                    }
                }
            } else if let Err(err) = state.changed().await {
                return Err(ClusterNodeError::Unexpected(err.into()));
            }
        }
    }

    async fn handle_timebound_request_vote_response(
        &self,
        timeout: time::Duration,
        response: &mut mpsc::Receiver<rpc::ServerResponse>,
    ) -> cluster_node::Result<()> {
        let current_term = self.current_term();
        let current_cluster_node_count = self.cluster_node_count();

        let mut total_votes_over_term = 0;

        loop {
            match time::timeout(timeout, response.recv()).await {
                Ok(Some(res)) => match res.body() {
                    rpc::ResponseBody::RequestVote { vote_granted } => {
                        if *vote_granted {
                            naive_logging::log(&self.id, "received vote for this term");
                            total_votes_over_term += 1;

                            if total_votes_over_term * 2 > current_cluster_node_count {
                                naive_logging::log(
                                    &self.id,
                                    &format!("won election for term {current_term}"),
                                );

                                if let Err(err) = self.upgrade_to_leader() {
                                    return Err(ClusterNodeError::Unexpected(err.into()));
                                }

                                return Ok(());
                            }
                        } else if let Err(err) = self.sync_term(res.term()).await {
                            return Err(ClusterNodeError::Unexpected(err.into()));
                        }
                    }
                    rpc::ResponseBody::AppendEntries { .. } => {
                        return Err(ClusterNodeError::Unexpected(anyhow::anyhow!(
                            "invalid response [AppendEntries] to RequestVote RPC"
                        )));
                    }
                },
                Ok(None) => continue,
                Err(err) => return Err(ClusterNodeError::Timeout(err)),
            }
        }
    }
}

#[cfg(test)]
mod tests {}
