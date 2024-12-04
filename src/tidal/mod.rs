pub mod auth;
pub mod data;

pub struct TidalClient {
    pub token: String,
}

impl TidalClient {
    pub fn new(token: String) -> Self {
        Self { token }
    }
}
