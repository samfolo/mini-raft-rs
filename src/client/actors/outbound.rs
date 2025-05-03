use tokio::{sync::mpsc, time};

use crate::{
    client::{self, RandomDataClient},
    timeout,
};

pub async fn run_outbound_actor(
    client: &RandomDataClient,
    publisher: mpsc::Sender<client::Message>,
) -> client::Result<()> {
    let timeout_range = timeout::TimeoutRange::new(
        client.min_request_interval_ms,
        client.max_request_interval_ms,
    );

    loop {
        match client.make_random_request(publisher.clone()).await {
            Ok(_) => time::sleep(timeout_range.random()).await,
            Err(err) => return Err(client::ClientRequestError::Unexpected(err.into())),
        }
    }
}
