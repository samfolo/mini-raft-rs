use anyhow::bail;
use tokio::time;

use crate::server::{Server, ServerState};

pub async fn run_follower_actor(
    server: &Server,
    cancellation_token: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let mut state = server.state.clone();

    let cancelled = cancellation_token.cancelled();
    tokio::pin!(cancelled);

    loop {
        if *state.borrow_and_update() == ServerState::Follower {
            println!("{}: FOLLOWER", server.id);
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
