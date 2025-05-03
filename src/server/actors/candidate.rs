use anyhow::bail;
use tokio::{sync::mpsc, task::JoinSet, time};

use crate::server::{self, Server, ServerState};

pub async fn run_candidate_actor(
    server: &Server,
    receiver: mpsc::Receiver<server::Message>,
    cancellation_token: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let mut state = server.state.clone();
    let server_id = server.id.clone();

    let cancelled = cancellation_token.cancelled();
    tokio::pin!(cancelled);

    loop {
        if *state.borrow_and_update() == ServerState::Candidate {
            let timeout = server.generate_random_timeout();

            let mut join_set = JoinSet::new();
            let current_term = server.current_term();

            for (_, peer_handle) in server.peer_list.peers_iter() {
                let peer_handle = peer_handle.clone();

                join_set.spawn(async move {
                    peer_handle
                        .request_vote(server_id, current_term)
                        .await
                        .map_err(|err| anyhow::anyhow!("failed to request vote: {err:?}"))
                });
            }

            let _ = join_set.join_all().await;

            // Wait for a message, or a random timeout
            // If message, reset the random timeout
            // else upgrade to candidate
            time::sleep(timeout).await;
        } else {
            tokio::select! {
              res = state.changed() => {
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
}
