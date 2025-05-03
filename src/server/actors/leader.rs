use anyhow::bail;
use tokio::{sync::mpsc, task::JoinSet, time};

use crate::{
    naive_logging,
    server::{self, Server, ServerState, request::ServerMessagePayload},
};

pub async fn run_leader_actor(
    server: &Server,
    mut receiver: mpsc::Receiver<server::Message>,
    cancellation_token: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let mut state = server.state.clone();
    let server_id = server.id.clone();

    let cancelled = cancellation_token.cancelled();
    tokio::pin!(cancelled);

    loop {
        if *state.borrow_and_update() == ServerState::Leader {
            let current_term = server.current_term();

            // timeout controls
            let timeout = time::sleep(server.heartbeat_interval);
            tokio::pin!(timeout);

            // append entries to followers
            let mut join_set = JoinSet::new();
            for (_, peer_handle) in server.peer_list.peers_iter() {
                let peer_handle = peer_handle.clone();

                join_set.spawn(async move {
                    peer_handle
                        .append_entries(server_id, current_term, vec![])
                        .await
                        .map_err(|err| anyhow::anyhow!("failed to append entries: {err:?}"))
                });
            }

            let _ = join_set.join_all().await;

            // commit tally

            'heartbeat: loop {
                tokio::select! {
                    _ = &mut timeout => {
                        break 'heartbeat;
                    }
                    res = state.changed() => {
                        if let Err(err) = res {
                            bail!("{:?}", err);
                        }
                        break 'heartbeat;
                    }
                    _ = &mut cancelled => {
                        return Ok(())
                    }
                    Some(msg) = receiver.recv() => {
                        match msg {
                            server::Message::Request(req) => {
                                let request_term = req.term();

                                match req.body() {
                                    server::ServerRequestBody::AppendEntries { leader_id, entries } => {
                                        naive_logging::log(
                                            &server.id,
                                            &format!(
                                                "<- APPEND_ENTRIES (req) {{ term: {request_term}, leader_id: {leader_id}, entries: {entries:?} }}"
                                            ),
                                        );

                                        todo!("fencing scenario");
                                    }
                                    server::ServerRequestBody::RequestVote { candidate_id } => {
                                        naive_logging::log(
                                            &server.id,
                                            &format!(
                                                "<- REQUEST_VOTE (req) {{ term: {request_term}, candidate_id: {candidate_id} }}"
                                            ),
                                        );

                                        todo!("fencing scenario");
                                    }
                                }
                            }
                            server::Message::Response(res) => match res.body() {
                                server::ServerResponseBody::AppendEntries { success } => {
                                    naive_logging::log(&server.id, &format!("<- APPEND_ENTRIES (res) {{ success: {success} }}"));
                                    if *success {
                                        // track the commit tally vector here
                                    }
                                },
                                server::ServerResponseBody::RequestVote { vote_granted } => {
                                  naive_logging::log(&server.id, &format!("<- REQUEST_VOTE (res) {{ vote_granted: {vote_granted} }}"));
                                  unreachable!("should never have received this message");
                                }
                            },
                        }
                    }
                }
            }
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
