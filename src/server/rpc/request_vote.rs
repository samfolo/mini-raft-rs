use tokio::sync::broadcast;
use tokio::sync::mpsc;

use crate::{naive_logging, server};
use server::{Server, rpc};

impl Server {
    /// `RequestVote` RPCs are initiated by candidates during elections.
    pub(in crate::server) fn request_vote(
        &self,
    ) -> anyhow::Result<
        mpsc::Receiver<rpc::ServerResponse>,
        broadcast::error::SendError<rpc::ServerRequest>,
    > {
        let current_term = self.current_term();

        naive_logging::log(
            self.id,
            &format!(
                "-> REQUEST_VOTE {{ term: {}, candidate_id: {} }}",
                current_term, self.id
            ),
        );

        let (responder, receiver) = mpsc::channel(Self::DEFAULT_MESSAGE_BUFFER_SIZE);

        self.publisher.send(rpc::ServerRequest::new(
            current_term,
            responder,
            rpc::RequestBody::RequestVote {
                candidate_id: self.id,
            },
        ))?;

        Ok(receiver)
    }
}

#[cfg(test)]
mod tests {
    use tokio::{sync::watch, time};

    use crate::timeout;

    use super::*;

    const TEST_CHANNEL_CAPACITY: usize = 16;

    #[test]
    fn starts() -> anyhow::Result<()> {
        let (publisher, _subscriber) = broadcast::channel(TEST_CHANNEL_CAPACITY);
        let (_, cluster_node_count) = watch::channel(1);

        let _ = Server::new(
            publisher,
            time::Duration::from_millis(5),
            timeout::TimeoutRange::new(10, 20),
            cluster_node_count,
        );

        Ok(())
    }
}
