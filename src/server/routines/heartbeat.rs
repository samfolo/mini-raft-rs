use std::pin;
use tokio::time;

use crate::{cluster_node, server};
use cluster_node::error::ClusterNodeError;
use server::{Server, ServerState};

impl Server {
    pub(in crate::server) async fn run_heartbeat_routine(&self) -> cluster_node::Result<()> {
        let id = self.id;
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
                    if *state.borrow() == ServerState::Leader {
                        if let Err(err) = self.append_entries(vec![]) {
                            return Err(ClusterNodeError::Heartbeat(id, err.into()))
                        }
                    }

                    reset_timeout(&mut timeout);
                },
                res = state.changed() => {
                    if let Err(err) = res {
                        return Err(ClusterNodeError::Unexpected(err.into()));
                    }

                    if *state.borrow() == ServerState::Leader {
                        timeout.as_mut().reset(time::Instant::now());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {}
