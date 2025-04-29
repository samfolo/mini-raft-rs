use std::pin;
use tokio::time;

use crate::{cluster_node, server};
use cluster_node::error::ClusterNodeError;
use server::{Server, ServerState};

impl Server {
    pub(in crate::server) async fn run_heartbeat_routine(&self) -> cluster_node::Result<()> {
        let mut state = self.state_tx.subscribe();

        let timeout = time::sleep(self.heartbeat_interval);
        tokio::pin!(timeout);

        let reset_timeout = |timeout: &mut pin::Pin<&mut time::Sleep>| {
            timeout
                .as_mut()
                .reset(time::Instant::now() + self.heartbeat_interval)
        };

        loop {
            tokio::select! {
                _ = &mut timeout => {
                    if *state.borrow_and_update() == ServerState::Leader {

                        match self.append_entries(self.log.entries_from(self.commit_index())) {
                            Ok(mut response) => {
                                while let Some(resp) = response.recv().await {
                                    println!("GOT res from node ID {}", resp.sender_id());
                                }
                            },
                            Err(err) => return Err(ClusterNodeError::Heartbeat(self.id, err.into()))
                        }
                    }

                    reset_timeout(&mut timeout);
                },
                res = state.changed() => {
                    if let Err(err) = res {
                        return Err(ClusterNodeError::Unexpected(err.into()));
                    }

                    if *state.borrow_and_update() == ServerState::Leader {
                        timeout.as_mut().reset(time::Instant::now());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {}
