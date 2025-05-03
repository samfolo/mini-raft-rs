use tokio::sync::mpsc;

use crate::{domain::node_id, message, naive_logging, state_machine};

use super::request;

#[derive(Debug, Clone)]
pub struct ClientHandle {
    sender: mpsc::Sender<message::Message>,
}

impl ClientHandle {
    pub fn new(sender: mpsc::Sender<message::Message>) -> Self {
        Self { sender }
    }

    pub async fn handle_client_response(
        &self,
        responder_id: node_id::NodeId,
        success: bool,
        snapshot: state_machine::InMemoryStateMachineSnapshot,
    ) -> anyhow::Result<(), mpsc::error::SendError<message::Message>> {
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
