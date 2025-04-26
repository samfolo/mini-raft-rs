use chrono::Utc;

use crate::domain;

pub fn log(id: &domain::node_id::NodeId, message: &str) {
    let time = Utc::now().time().to_string();
    println!("{} {}: {}", &time[..12], id, message);
}
