use crate::spotify::SpotifyClient;
use reqwest::Client;
use serde::{Serialize,Deserialize};
use serde_json::Value;

// TODO: Bypass 100 track limit with pagination, 

#[derive(Deserialize, Debug)]
pub struct SpotifyUser {
    pub country: String,
    pub display_name: String,
    pub email: String,
    pub explicit_content: ExplicitContent,
    pub external_urls: ExternalUrls,
    pub followers: Followers,
    pub href: String,
    pub id: String,
    pub images: Vec<Image>,
    pub product: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub uri: String,
}

#[derive(Deserialize, Debug)]
pub struct ExplicitContent {
    pub filter_enabled: bool,
    pub filter_locked: bool,
}

#[derive(Deserialize, Debug)]
pub struct ExternalUrls {
    pub spotify: String,
}

#[derive(Deserialize, Debug)]
pub struct Followers {
    pub href: Option<String>,
    pub total: u32,
}

#[derive(Deserialize, Debug)]
pub struct Image {
    pub url: String,
    pub height: Option<u32>,
    pub width: Option<u32>,
}

#[derive(Serialize)]
struct CreatePlaylistRequest {
    name: String,
    description: String,
    public: bool,
}

pub async fn create_playlist(client: &SpotifyClient, name: &str, description: &str, public: bool) -> Result<String, Box<dyn std::error::Error>> {
    let user_id = get_current_user(client).await?.id;
    let url = format!("https://api.spotify.com/v1/users/{}/playlists", user_id);
    let request_body = CreatePlaylistRequest {
        name: name.to_string(),
        description: description.to_string(),
        public,
    };

    let response = Client::new()
        .post(&url)
        .bearer_auth(&client.token)
        .json(&request_body)
        .send()
        .await?
        .json::<Value>()
        .await?;

    let playlist_id = response["id"].as_str().ok_or("Failed to get playlist ID")?.to_string();
    Ok(playlist_id)
}

pub async fn add_tracks_to_playlist(client: &SpotifyClient, playlist_id: &str, isrcs: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut track_uris = Vec::new();
    for isrc in isrcs {
        let track_uri = get_track_uri_from_isrc(client, &isrc).await?;
        track_uris.push(track_uri);
    }

    let url = format!("https://api.spotify.com/v1/playlists/{}/tracks", playlist_id);
    let response = Client::new()
        .post(&url)
        .bearer_auth(&client.token)
        .json(&serde_json::json!({ "uris": track_uris }))
        .send()
        .await?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Failed to add tracks to playlist")))
    }
}

pub async fn fetch_spotify_playlist(client: &SpotifyClient, playlist_id: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let url = format!("https://api.spotify.com/v1/playlists/{}", playlist_id);
    let response = Client::new()
        .get(&url)
        .bearer_auth(&client.token)
        .send()
        .await?
        .json::<Value>()
        .await?;
    Ok(response)
}

async fn get_current_user(client: &SpotifyClient) -> Result<SpotifyUser, Box<dyn std::error::Error>> {
    let url = "https://api.spotify.com/v1/me";
    let response = Client::new()
        .get(url)
        .bearer_auth(&client.token)
        .send()
        .await?
        .json::<SpotifyUser>()
        .await?;
    
    Ok(response)
}

async fn get_track_uri_from_isrc(client: &SpotifyClient, isrc: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("https://api.spotify.com/v1/search?q=isrc:{}&type=track", isrc);
    let response = Client::new()
        .get(&url)
        .bearer_auth(&client.token)
        .send()
        .await?
        .json::<Value>()
        .await?;

    let track_uri = response["tracks"]["items"][0]["uri"]
        .as_str()
        .ok_or("Failed to get track URI")?
        .to_string();
    Ok(track_uri)
}