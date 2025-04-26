use crate::{
    client,
    cluster_node::{self, error::ClusterNodeError},
    naive_logging, server,
};
use server::{Server, ServerState};

impl Server {
    /// The leader handles all client requests; if a client contacts a follower, the
    /// follower redirects it to the leader.
    pub(in crate::server) async fn handle_client_request(&self) -> cluster_node::Result<()> {
        let state = self.state_tx.subscribe();

        let mut subscriber = match self.client_conn.upgrade() {
            Some(tx) => tx,
            None => {
                return Err(ClusterNodeError::Unexpected(anyhow::anyhow!(
                    "client connection closed."
                )));
            }
        }
        .subscribe();

        loop {
            match subscriber.recv().await {
                Ok(request) => {
                    if *state.borrow() == ServerState::Leader {
                        match request {
                            client::ClientRequest { responder, body } => {
                                naive_logging::log(
                                    &self.id,
                                    &format!("{} <- CLIENT_REQUEST: {}", self.listener, body),
                                );
                            }
                        }
                    }
                }
                Err(err) => return Err(ClusterNodeError::Unexpected(err.into())),
            }
        }
    }
}

#[cfg(test)]
mod tests {}
