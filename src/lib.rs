pub fn does() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_does() {
        assert!(does());
    }
}
