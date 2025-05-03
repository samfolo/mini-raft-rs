use tokio::sync::mpsc;

use crate::{client, domain::node_id, naive_logging, state_machine};

use super::request;

#[derive(Debug, Clone)]
pub struct ClientHandle {
    sender: mpsc::Sender<client::Message>,
}

impl ClientHandle {
    pub fn new(sender: mpsc::Sender<client::Message>) -> Self {
        Self { sender }
    }

    pub async fn handle_client_response(
        &self,
        responder_id: node_id::NodeId,
        success: bool,
        snapshot: state_machine::InMemoryStateMachineSnapshot,
    ) -> anyhow::Result<(), mpsc::error::SendError<client::Message>> {
        naive_logging::log(
            &responder_id,
            &format!("-> CLIENT_UPDATE_CMD (res) {{ success: {success}, snapshot: {snapshot} }}"),
        );

        self.sender
            .send(request::ClientResponse::new(success, snapshot).into())
            .await?;

        Ok(())
    }
}
