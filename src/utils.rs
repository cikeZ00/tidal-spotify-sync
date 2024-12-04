pub fn _log_error<E: std::error::Error>(error: E) {
    eprintln!("Error: {}", error);
}
