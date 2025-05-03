use futures_util::StreamExt;
use tokio::sync::mpsc;

use crate::{
    client::{self, RandomDataClient, receiver},
    naive_logging,
};

pub async fn run_inbound_actor(
    client: &RandomDataClient,
    receiver: mpsc::Receiver<client::Message>,
) -> client::Result<()> {
    let mut receiver = receiver::ClientReceiver::new(receiver);

    let stream = async_stream::stream! {
        while let Some(item) = receiver.recv().await {
            yield item;
        }
    };
    futures_util::pin_mut!(stream);

    while let Some(message) = stream.next().await {
        match message {
            client::Message::Request(req) => {
                match req.body {
                    client::ClientRequestBody::Read => {
                        naive_logging::log(&client.id, "<- CLIENT_READ_CMD (req) {{  }}")
                    }
                    client::ClientRequestBody::Write { command } => naive_logging::log(
                        &client.id,
                        &format!("<- CLIENT_WRITE_CMD (req) {{ command: {command} }}"),
                    ),
                }

                unreachable!("should never have received this message")
            }
            client::Message::Response(res) => match res.body {
                client::ClientResponseBody::Read { snapshot } => naive_logging::log(
                    &client.id,
                    &format!("<- CLIENT_READ_CMD (res) {{ snapshot: {snapshot} }}"),
                ),
                client::ClientResponseBody::Write { success } => naive_logging::log(
                    &client.id,
                    &format!("<- CLIENT_WRITE_CMD (res) {{ success: {success} }}"),
                ),
            },
        }
    }

    Ok(())
}
