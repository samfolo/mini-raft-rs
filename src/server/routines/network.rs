use crate::{
    client,
    cluster_node::{self, error::ClusterNodeError},
    naive_logging, server,
};
use server::{Server, ServerState};

impl Server {
    ///Once a leader has been elected, it begins servicing client requests. Each client
    /// request contains a command to be executed by the replicated state machines.
    ///
    /// The leader appends the command to its log as a new entry, then issues
    /// `AppendEntries` RPCs in parallel to each of the other servers to replicate the
    /// entry.
    ///
    /// The leader handles all client requests; if a client contacts a follower, the
    /// follower redirects it to the leader.
    pub(in crate::server) async fn handle_client_request(&self) -> cluster_node::Result<()> {
        let mut state = self.state_tx.subscribe();

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
            if *state.borrow_and_update() == ServerState::Leader {
                loop {
                    tokio::select! {
                        req = subscriber.recv() => {
                            match req {
                                Ok(request) => {
                                    let client::ClientRequest { responder, body } =  request;

                                    naive_logging::log(
                                        &self.id,
                                        &format!("{} <- CLIENT_REQUEST: {}", self.listener, body),
                                    );

                                    self.append_to_log(body);

                                    if let Err(err) = responder.send(client::ClientResponse::new(true)).await {
                                        return Err(ClusterNodeError::Unexpected(err.into()));
                                    }
                                }
                                Err(err) => return Err(ClusterNodeError::Unexpected(err.into())),
                            }
                        },
                        res = state.changed() => {
                            if let Err(err) = res {
                                return Err(ClusterNodeError::Unexpected(err.into()));
                            }
                        }
                    }
                }
            } else if let Err(err) = state.changed().await {
                return Err(ClusterNodeError::Unexpected(err.into()));
            }
        }
    }
}

#[cfg(test)]
mod tests {}
