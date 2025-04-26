use std::fmt;

use tokio::sync::mpsc;

/// StateKey represents the location of the target state client request was made to update.
#[derive(Debug, Clone, Copy)]
pub enum StateKey {
    X,
    Y,
    Z,
}

/// Op represents the operation to be taken on the target state.
#[derive(Debug, Clone, Copy)]
pub enum Op {
    Increment,
    Decrement,
    Replace,
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Op::Increment => "+=",
                Op::Decrement => "-=",
                Op::Replace => "<=",
            }
        )
    }
}

/// ClientRequestBody represents the body of a ClientRequest.
#[derive(Debug, Clone, Copy)]
pub struct ClientRequestBody {
    op: Op,
    key: StateKey,
    value: i64,
}

impl ClientRequestBody {
    pub fn new(op: Op, key: StateKey, value: i64) -> Self {
        Self { op, key, value }
    }

    pub fn op(&self) -> Op {
        self.op
    }

    pub fn key(&self) -> StateKey {
        self.key
    }

    pub fn value(&self) -> i64 {
        self.value
    }
}

impl fmt::Display for ClientRequestBody {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{{ command: [ State.{:?} {} {} ] }}",
            self.key, self.op, self.value
        )
    }
}

/// ClientRequest represents a request sent by a Client.
#[derive(Debug, Clone)]
pub struct ClientRequest {
    pub responder: mpsc::Sender<ClientResponse>,
    pub body: ClientRequestBody,
}

impl ClientRequest {
    pub fn new(responder: mpsc::Sender<ClientResponse>, body: ClientRequestBody) -> Self {
        Self { responder, body }
    }

    pub fn body(&self) -> &ClientRequestBody {
        &self.body
    }

    pub fn can_respond(&self) -> bool {
        !self.responder.is_closed()
    }

    pub async fn respond(
        self,
        success: bool,
    ) -> Result<(), mpsc::error::SendError<ClientResponse>> {
        self.responder.send(ClientResponse { success }).await
    }
}

/// ClientResponse represents a response received from a Client.
#[derive(Debug, Clone, Copy)]
pub struct ClientResponse {
    success: bool,
}

impl ClientResponse {
    pub fn new(success: bool) -> Self {
        Self { success }
    }

    pub fn success(&self) -> bool {
        self.success
    }
}
