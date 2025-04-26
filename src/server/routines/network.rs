use crate::{
    client::{self, error::ClientRequestError},
    naive_logging, server,
};
use server::{Server, ServerState};

impl Server {
    /// The leader handles all client requests; if a client contacts a follower, the
    /// follower redirects it to the leader.
    pub(in crate::server) async fn handle_client_request(&mut self) -> client::Result<()> {
        let state = self.state_tx.subscribe();

        loop {
            match self.client_recv.recv().await {
                Ok(request) => {
                    if *state.borrow() == ServerState::Leader {
                        match request {
                            client::ClientRequest { responder, body } => {
                                naive_logging::log(
                                    &self.id,
                                    &format!("received client request: {body:?}"),
                                );
                            }
                        }
                    }
                }
                Err(err) => return Err(ClientRequestError::Unexpected(err.into())),
            }
        }
    }
}

#[cfg(test)]
mod tests {}
