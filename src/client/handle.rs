use tokio::sync::mpsc;

use crate::{client, domain::node_id, naive_logging, state_machine};

use super::request::{self, ClientResponseBody};

#[derive(Debug, Clone)]
pub struct ClientHandle {
    sender: mpsc::Sender<client::Message>,
}

impl ClientHandle {
    pub fn new(sender: mpsc::Sender<client::Message>) -> Self {
        Self { sender }
    }

    pub async fn handle_client_read_response(
        &self,
        responder_id: node_id::NodeId,
        snapshot: state_machine::InMemoryStateMachineSnapshot,
    ) -> anyhow::Result<(), mpsc::error::SendError<client::Message>> {
        naive_logging::log(
            &responder_id,
            &format!("-> CLIENT_READ_CMD (res) {{ snapshot: {snapshot} }}"),
        );

        self.sender
            .send(request::ClientResponse::new(ClientResponseBody::Read { snapshot }).into())
            .await?;

        Ok(())
    }

    pub async fn handle_client_write_response(
        &self,
        responder_id: node_id::NodeId,
        success: bool,
    ) -> anyhow::Result<(), mpsc::error::SendError<client::Message>> {
        naive_logging::log(
            &responder_id,
            &format!("-> CLIENT_WRITE_CMD (res) {{ success: {success} }}"),
        );

        self.sender
            .send(request::ClientResponse::new(ClientResponseBody::Write { success }).into())
            .await?;

        Ok(())
    }
}
