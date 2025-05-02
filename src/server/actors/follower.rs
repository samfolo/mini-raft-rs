use anyhow::bail;
use tokio::{sync::mpsc, time};

use crate::server::{self, Server, ServerState};

pub async fn run_follower_actor(
    server: &Server,
    receiver: mpsc::Receiver<server::ServerRequest>,
    cancellation_token: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let mut state = server.state.clone();

    let cancelled = cancellation_token.cancelled();
    tokio::pin!(cancelled);

    loop {
        if *state.borrow_and_update() == ServerState::Follower {
            println!("{}: FOLLOWER", server.id);
            let timeout = server.generate_random_timeout();

            // Wait for a message, or a random timeout
            // If message, reset the random timeout
            // else upgrade to candidate
            time::sleep(time::Duration::from_millis(1000)).await;
        } else {
            let mut state_changed = state.changed();

            tokio::select! {
              Err(err) = state_changed => {
                bail!("{:?}", err);
              }
              _ = cancelled => {
                return Ok(())
              }
            }
        }
    }

    Ok(())
}
