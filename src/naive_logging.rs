pub fn log(id: uuid::Uuid, message: &str) {
    println!("[{}]: {}", id, message);
}
