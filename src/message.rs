use crate::{client, server};

/// Message represents a message sent from or received by a Server.
#[derive(Clone, Debug)]
pub enum Message {
    ClientRequest(client::ClientRequest),
    ClientResponse(client::ClientResponse),
    ServerRequest(server::ServerRequest),
    ServerResponse(server::ServerResponse),
}

impl From<client::ClientRequest> for Message {
    fn from(req: client::ClientRequest) -> Self {
        Self::ClientRequest(req)
    }
}

impl From<client::ClientResponse> for Message {
    fn from(res: client::ClientResponse) -> Self {
        Self::ClientResponse(res)
    }
}

impl From<server::ServerRequest> for Message {
    fn from(req: server::ServerRequest) -> Self {
        Self::ServerRequest(req)
    }
}

impl From<server::ServerResponse> for Message {
    fn from(res: server::ServerResponse) -> Self {
        Self::ServerResponse(res)
    }
}
