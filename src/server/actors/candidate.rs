use std::pin;

use anyhow::bail;
use tokio::{
    sync::mpsc,
    task::JoinSet,
    time::{self, Sleep},
};

use crate::{
    naive_logging,
    server::{self, Server, ServerState, request::ServerMessagePayload},
};

pub async fn run_candidate_actor(
    server: &Server,
    mut receiver: mpsc::Receiver<server::Message>,
    cancellation_token: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let mut state = server.state.clone();
    let server_id = server.id.clone();

    let cancelled = cancellation_token.cancelled();
    tokio::pin!(cancelled);

    loop {
        if *state.borrow_and_update() == ServerState::Candidate {
            // timeout controls
            let timeout_dur = server.generate_random_timeout();
            let timeout = time::sleep(timeout_dur);
            tokio::pin!(timeout);

            let reset_timeout = |timeout: &mut pin::Pin<&mut Sleep>| {
                timeout.set(time::sleep(server.generate_random_timeout()))
            };

            // increment term
            server.set_current_term(|prev| prev + 1);
            let current_term = server.current_term();

            // vote for self
            server.set_voted_for(Some(server_id));

            // request votes for term
            let mut join_set = JoinSet::new();
            for (_, peer_handle) in server.peer_list.peers_iter() {
                let peer_handle = peer_handle.clone();

                join_set.spawn(async move {
                    peer_handle
                        .request_vote(server_id, current_term)
                        .await
                        .map_err(|err| anyhow::anyhow!("failed to request vote: {err:?}"))
                });
            }

            let acks = join_set.join_all().await;

            // tally
            let total_votes_requested = acks.len();
            let mut total_votes_over_term = 0;

            'election: loop {
                tokio::select! {
                    _ = &mut timeout => {
                        naive_logging::log(&server.id, "failed to receive enough votes this term.");
                        naive_logging::log(&server.id, "restarting election...");
                        break 'election;
                    }
                    res = state.changed() => {
                        if let Err(err) = res {
                            bail!("{:?}", err);
                        }
                        break 'election;
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

                                        let sender_handle = server.peer_list.get(&req.sender_id()).unwrap();

                                        // verify terms then downgrade...
                                        if let Err(err) = server.downgrade_to_follower() {
                                            bail!("failed to downgrade to follower: {err:?}");
                                        }
                                    }
                                    server::ServerRequestBody::RequestVote { candidate_id } => {
                                        naive_logging::log(
                                            &server.id,
                                            &format!(
                                                "<- REQUEST_VOTE (req) {{ term: {request_term}, candidate_id: {candidate_id} }}"
                                            ),
                                        );

                                        let sender_handle = server.peer_list.get(&req.sender_id()).unwrap();

                                        let vote_granted = request_term >= current_term && server.voted_for().is_none();
                                        if vote_granted {
                                            naive_logging::log(
                                                &server.id,
                                                &format!(
                                                    "granting vote to candidate... {{ current_term: {current_term}, request_term: {request_term}, voted_for: {:?} }}",
                                                    server.voted_for(),
                                                ),
                                            );

                                            server.set_current_term(|_| request_term);
                                            server.set_voted_for(Some(*candidate_id));
                                            if let Err(err) = server.downgrade_to_follower() {
                                                bail!("failed to downgrade to follower: {err:?}");
                                            }
                                            sender_handle.request_vote_response(server_id, current_term, true).await?;
                                        } else {
                                            naive_logging::log(
                                                &server.id,
                                                &format!(
                                                    "refusing vote for candidate... {{ current_term: {current_term}, request_term: {request_term}, voted_for: {:?} }}",
                                                    server.voted_for(),
                                                ),
                                            );

                                            sender_handle.request_vote_response(server_id, current_term, false).await?;
                                        }
                                    }
                                }
                            }
                            server::Message::Response(res) => match res.body() {
                                server::ServerResponseBody::AppendEntries {} => unreachable!(),
                                server::ServerResponseBody::RequestVote { vote_granted } => {
                                    naive_logging::log(
                                        &server.id,
                                        &format!("<- REQUEST_VOTE (res) {{ vote_granted: {vote_granted} }}"),
                                    );
                                    if *vote_granted {
                                        total_votes_over_term += 1;

                                        if total_votes_over_term * 2 > total_votes_requested {
                                            naive_logging::log(&server.id, "received a majority of votes this term.");
                                            if let Err(err) = server.upgrade_to_leader() {
                                                bail!("failed to upgrade to leader: {err:?}");
                                            }
                                            break 'election;
                                        }
                                    }
                                }
                            },
                        }

                        // decide when this happens:
                        reset_timeout(&mut timeout);
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
