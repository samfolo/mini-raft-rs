use crate::{client, server};

/// Message represents a message sent from or received by an actor.
#[derive(Debug, Clone)]
pub enum Message {
    Client(client::Message),
    Server(server::Message),
}

impl From<client::ClientRequest> for Message {
    fn from(req: client::ClientRequest) -> Self {
        Self::Client(client::Message::Request(req))
    }
}

impl From<client::ClientResponse> for Message {
    fn from(res: client::ClientResponse) -> Self {
        Self::Client(client::Message::Response(res))
    }
}

impl From<server::ServerRequest> for Message {
    fn from(req: server::ServerRequest) -> Self {
        Self::Server(server::Message::Request(req))
    }
}

impl From<server::ServerResponse> for Message {
    fn from(res: server::ServerResponse) -> Self {
        Self::Server(server::Message::Response(res))
    }
}
