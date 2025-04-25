use chrono::Utc;

pub fn log(id: uuid::Uuid, message: &str) {
    let time = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    println!("[{time}] [{}]: {}", id, message);
}
