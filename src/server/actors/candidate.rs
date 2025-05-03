use anyhow::bail;
use tokio::{sync::mpsc, task::JoinSet, time};

use crate::{
    domain::node_id,
    server::{self, Server, ServerState},
};

pub async fn run_candidate_actor(
    server: &Server,
    receiver: mpsc::Receiver<server::ServerRequest>,
    cancellation_token: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let mut state = server.state.clone();
    let server_id = server.id.clone();

    let cancelled = cancellation_token.cancelled();
    tokio::pin!(cancelled);

    let request_vote_cb = async move |id: node_id::NodeId, handle: server::ServerHandle| {
        handle
            .request_vote(server.id.clone(), server.current_term())
            .await
            .map_err(|err| anyhow::anyhow!("failed to request vote: {err:?}"))
    };

    loop {
        if *state.borrow_and_update() == ServerState::Candidate {
            let timeout = server.generate_random_timeout();

            let mut join_set = JoinSet::new();
            let current_term = server.current_term();

            for (peer_id, peer_handle) in server.peer_list.peers_iter() {
                let peer_id = peer_id.clone();
                let peer_handle = peer_handle.clone();

                join_set.spawn(async move {
                    if let Err(err) = peer_handle
                        .request_vote(server_id.clone(), current_term)
                        .await
                    {
                        bail!("failed to request vote: {err:?}")
                    }

                    Ok(())
                });
            }

            let results = join_set.join_all().await;

            // Wait for a message, or a random timeout
            // If message, reset the random timeout
            // else upgrade to candidate
            time::sleep(timeout).await;
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
