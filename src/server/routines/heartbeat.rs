// use futures_util::stream::StreamExt;
// use std::pin;
// use tokio::{sync::mpsc, time};

// use crate::{cluster_node, server};
// use cluster_node::error::ClusterNodeError;
// use server::{Server, ServerState};

// impl Server {
//     pub(in crate::server) async fn run_heartbeat_routine(&self) -> cluster_node::Result<()> {
//         let mut state = self.state_tx.subscribe();

//         let timeout = time::sleep(self.heartbeat_interval);
//         tokio::pin!(timeout);

//         let reset_timeout = |timeout: &mut pin::Pin<&mut time::Sleep>| {
//             timeout
//                 .as_mut()
//                 .reset(time::Instant::now() + self.heartbeat_interval)
//         };

//         let (responder, mut receiver) = mpsc::channel(32);

//         let mut stream = async_stream::stream! {
//             while let Some(item) = receiver.recv().await {
//                 yield item;
//             }
//         };
//         futures_util::pin_mut!(stream);

//         loop {
//             tokio::select! {
//                 response = &mut stream.next() => {
//                     println!("STREAM: {:?}", response);
//                 },
//                 _ = &mut timeout => {
//                     if *state.borrow_and_update() == ServerState::Leader {
//                         if let Err(err) = self.append_entries(responder.clone(), self.log.entries_from(self.commit_index())) {
//                             return Err(ClusterNodeError::Heartbeat(self.id, err.into()));
//                         }
//                     }

//                     reset_timeout(&mut timeout);
//                 },
//                 res = state.changed() => {
//                     if let Err(err) = res {
//                         return Err(ClusterNodeError::Unexpected(err.into()));
//                     }

//                     if *state.borrow_and_update() == ServerState::Leader {
//                         timeout.as_mut().reset(time::Instant::now());
//                     }
//                 }
//             }
//         }
//     }
// }

// #[cfg(test)]
// mod tests {}
