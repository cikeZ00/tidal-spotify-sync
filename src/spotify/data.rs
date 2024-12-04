use crate::spotify::SpotifyClient;
use reqwest::Client;
use rusqlite::Connection;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;
use crate::db::{insert_blacklist, is_blacklisted};

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

const SPOTIFY_API: &str = "https://api.spotify.com/v1";

async fn send_request_with_rate_limit(request: reqwest::RequestBuilder) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
    loop {
        let response = request.try_clone().unwrap().send().await?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            if let Some(retry_after) = response.headers().get(reqwest::header::RETRY_AFTER) {
                let retry_after_secs = retry_after.to_str()?.parse::<u64>()?;
                println!("Rate limited. Retrying after {} seconds...", retry_after_secs);
                sleep(Duration::from_secs(retry_after_secs)).await;
            } else {
                println!("Rate limited. Retrying after default 1 second...");
                sleep(Duration::from_secs(1)).await;
            }
        } else {
            return Ok(response);
        }
    }
}

pub async fn create_playlist(client: &SpotifyClient, name: &str, description: &str, public: bool) -> Result<String, Box<dyn std::error::Error>> {
    let user_id = get_current_user(client).await?.id;
    let url = format!("{}/users/{}/playlists", SPOTIFY_API, user_id);
    let request_body = CreatePlaylistRequest {
        name: name.to_string(),
        description: description.to_string(),
        public,
    };

    let request = Client::new()
        .post(&url)
        .bearer_auth(&client.token)
        .json(&request_body);

    let response = send_request_with_rate_limit(request).await?;
    let response_json = response.json::<Value>().await?;
    let playlist_id = response_json["id"].as_str().ok_or("Failed to get playlist ID")?.to_string();
    Ok(playlist_id)
}

pub async fn add_tracks_to_playlist(client: &SpotifyClient, playlist_id: &str, isrcs: Vec<String>, conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    let mut track_uris = Vec::new();
    for isrc in isrcs {
        // Ignore blacklisted irscs
        if is_blacklisted(conn, &isrc)? {
            println!("Track with ISRC {} is blacklisted, skipping.", isrc);
            continue;
        }
        match get_track_uri_from_isrc(client, &isrc).await? {
            Some(track_uri) => track_uris.push(track_uri),
            None => {
                println!("Track with ISRC {} not found, skipping.", isrc);
                insert_blacklist(conn, &isrc)?;
            }
        }
    }

    // Filter out blacklisted tracks
    track_uris.retain(|uri| !is_blacklisted(conn, uri).expect("Failed to check blacklist"));

    if track_uris.is_empty() {
        println!("No valid tracks found to add to playlist, skipping.");
        return Ok(());
    }

    let url = format!("{}/playlists/{}/tracks", SPOTIFY_API, playlist_id);
    let request = Client::new()
        .post(&url)
        .bearer_auth(&client.token)
        .json(&serde_json::json!({ "uris": track_uris }));

    let response = send_request_with_rate_limit(request).await?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Failed to add tracks to playlist")))
    }
}

pub async fn fetch_spotify_playlist(client: &SpotifyClient, playlist_id: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let url = format!("{}/playlists/{}", SPOTIFY_API, playlist_id);
    let request = Client::new()
        .get(&url)
        .bearer_auth(&client.token);

    let response = send_request_with_rate_limit(request).await?;
    let response_json = response.json::<Value>().await?;
    Ok(response_json)
}

async fn get_current_user(client: &SpotifyClient) -> Result<SpotifyUser, Box<dyn std::error::Error>> {
    let url = format!("{}/me", SPOTIFY_API);
    let request = Client::new()
        .get(url)
        .bearer_auth(&client.token);

    let response = send_request_with_rate_limit(request).await?;
    let user = response.json::<SpotifyUser>().await?;
    Ok(user)
}

async fn get_track_uri_from_isrc(client: &SpotifyClient, isrc: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let url = format!("{}/search?q=isrc:{}&type=track", SPOTIFY_API, isrc);
    let request = Client::new()
        .get(&url)
        .bearer_auth(&client.token);

    let response = send_request_with_rate_limit(request).await?;
    let response_json = response.json::<Value>().await?;

    println!("Got Track: {:?}", response_json);

    if let Some(track_uri) = response_json["tracks"]["items"].get(0).and_then(|item| item["uri"].as_str()) {
        Ok(Some(track_uri.to_string()))
    } else {
        Ok(None)
    }
}