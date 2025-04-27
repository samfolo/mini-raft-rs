use std::fmt;

use chrono::Utc;

pub fn log(id: &impl fmt::Display, message: &str) {
    let time = Utc::now().time().to_string();
    println!("{} {}: {}", &time[..12], id, message);
}
