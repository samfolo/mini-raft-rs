use std::pin;

use anyhow::bail;
use tokio::{
    sync::mpsc,
    time::{self, Sleep},
};

use crate::server::{self, Server, ServerState};

pub async fn run_follower_actor(
    server: &Server,
    mut receiver: mpsc::Receiver<server::ServerRequest>,
    cancellation_token: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let mut state = server.state.clone();

    let cancelled = cancellation_token.cancelled();
    tokio::pin!(cancelled);

    let recv = receiver.recv();
    tokio::pin!(recv);

    let mut timeout_dur = server.generate_random_timeout();
    let mut timeout = time::sleep(timeout_dur);
    tokio::pin!(timeout);
    let mut reset_timeout = |timeout: &mut pin::Pin<&mut Sleep>| {
        timeout.set(time::sleep(server.generate_random_timeout()))
    };

    loop {
        if *state.borrow_and_update() == ServerState::Follower {
            println!("{}: FOLLOWER", server.id);
            let mut state_changed = state.changed();

            // Wait for a message, or a random timeout
            // If message, reset the random timeout
            // else upgrade to candidate
            tokio::select! {
              _ = &mut timeout => {
                if let Err(err) = server.upgrade_to_candidate() {
                  bail!("failed to upgrade to candidate: {err:?}");
                }
              }
              msg = &mut recv => {
                reset_timeout(&mut timeout); // this might be an internal channel...
              }
              res = state_changed => {
                if let Err(err) = res {
                  bail!("{:?}", err);
                }
              }
              _ = &mut cancelled => {
                return Ok(())
              }
            }
        } else {
            let mut state_changed = state.changed();

            tokio::select! {
              res = state_changed => {
                if let Err(err) = res {
                  bail!("{:?}", err);
                }
              }
              _ = &mut cancelled => {
                return Ok(())
              }
            }
        }
    }

    Ok(())
}
