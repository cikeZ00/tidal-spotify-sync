mod tidal;
mod spotify;
mod sync;
mod config;
mod utils;
use env_logger;


#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::init();

    // Load configuration
    let config = config::load_config().expect("Failed to load configuration");

    // Authenticate with Tidal and Spotify
    let tidal_client = tidal::auth::authenticate(&config).await.unwrap();
    let spotify_client = spotify::auth::authenticate(&config).await.unwrap();

    // Perform sync
    sync::sync_data(&tidal_client, &spotify_client).await.unwrap();
}
