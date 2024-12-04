pub mod auth;
pub mod data;

pub struct SpotifyClient {
    pub token: String, // Add other fields if necessary
}

impl SpotifyClient {
    pub fn new(token: String) -> Self {
        Self { token }
    }
}
