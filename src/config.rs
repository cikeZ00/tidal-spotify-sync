use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use toml;

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub tidal: TidalConfig,
    pub spotify: SpotifyConfig,
}

#[derive(Deserialize, Serialize)]
pub struct TidalConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Deserialize, Serialize)]
pub struct SpotifyConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let config_path = "config.toml";

    if !std::path::Path::new(config_path).exists() {
        let default_config = Config {
            tidal: TidalConfig {
                client_id: "your_tidal_client_id".to_string(),
                client_secret: "your_tidal_client_secret".to_string(),
                redirect_uri: "http://localhost:8080".to_string(),
            },
            spotify: SpotifyConfig {
                client_id: "your_spotify_client_id".to_string(),
                client_secret: "your_spotify_client_secret".to_string(),
                redirect_uri: "http://localhost:8080".to_string(),
            },
        };

        let toml_string = toml::to_string_pretty(&default_config)?;

        let mut file = fs::File::create(config_path)?;
        file.write_all(toml_string.as_bytes())?;

        return Err("Configuration file not found. A default 'config.toml' has been created. Please update it with your credentials.".to_string()
            .into());
    }

    // Read and parse the existing config file
    let config_str = fs::read_to_string(config_path)?;
    let config: Config = toml::from_str(&config_str)?;
    Ok(config)
}
