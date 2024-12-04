use reqwest::Client;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use crate::tidal::TidalClient;
use serde_json::Value;
use tokio::time::{sleep, Duration};
use chrono::NaiveDateTime;
use crate::db::get_playlist_last_modified;
use rusqlite::{params, Connection};

// TODO: Make data we dont store optional in the Structs
#[derive(Deserialize, Debug)]
pub struct TidalPlaylist {
    pub id: String,
    pub name: String,
    pub tracks: Vec<TidalTrack>,
    pub last_updated: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct TidalTrack {
    pub id: String,
    pub attributes: TrackAttributes,
    pub relationships: TrackRelationships,
    pub links: TrackLinks,
}

#[derive(Deserialize, Debug)]
pub struct TrackAttributes {
    pub title: String,
    pub isrc: String,
    pub duration: String,
    pub explicit: bool,
    pub popularity: f64,
    pub availability: Vec<String>,
    #[serde(rename = "mediaTags")]
    pub media_tags: Vec<String>,
    #[serde(rename = "externalLinks")]
    pub external_links: Vec<ExternalLink>,
    pub copyright: String,
}

#[derive(Deserialize, Debug)]
pub struct ExternalLink {
    pub href: String,
    pub meta: ExternalLinkMeta,
}

#[derive(Deserialize, Debug)]
pub struct ExternalLinkMeta {
    pub r#type: String,
}

#[derive(Deserialize, Debug)]
pub struct TrackRelationships {
    pub albums: RelationshipLinks,
    pub artists: RelationshipLinks,
    pub providers: RelationshipLinks,
    pub radio: RelationshipLinks,
    #[serde(rename = "similarTracks")]
    pub similar_tracks: RelationshipLinks,
}

#[derive(Deserialize, Debug)]
pub struct RelationshipLinks {
    pub links: RelationshipSelfLink,
}

#[derive(Deserialize, Debug)]
pub struct RelationshipSelfLink {
    #[serde(rename = "self")]
    pub self_link: String,
}

#[derive(Deserialize, Debug)]
pub struct TrackLinks {
    #[serde(rename = "self")]
    pub self_link: String,
}

const TIDAL_API: &str = "https://openapi.tidal.com/v2";

pub async fn fetch_playlists(client: &TidalClient, conn: &Connection) -> Result<Vec<TidalPlaylist>, Box<dyn std::error::Error>> {
    let response = Client::new()
        .get(format!("{}/playlists/me", TIDAL_API))
        .bearer_auth(&client.token)
        .send()
        .await?;

    if response.status().is_success() {
        let response_headers = response.headers().clone();
        let response_body = response.text().await?;
        if response_body.is_empty() {
            return Err("Empty response body".into());
        }

        let response_json: Value = serde_json::from_str(&response_body)?;

        let mut playlists = Vec::new();
        let mut remaining_tokens = get_remaining_tokens(&response_headers);
        let replenish_rate = get_replenish_rate(&response_headers);
        let _burst_capacity = get_burst_capacity(&response_headers);

        if let Some(data) = response_json["data"].as_array() {
            for playlist in data {
                let id = playlist["id"].as_str().unwrap_or_default().to_string();
                let name = playlist["attributes"]["name"].as_str().unwrap_or_default().to_string();
                let last_updated = playlist["attributes"]["lastModifiedAt"].as_str().map(|s| s.to_string());

                // Check if the playlist should be skipped based on the last updated date
                if let Some(last_updated_str) = &last_updated {
                    println!("Last updated date: {:?}", last_updated_str);
                    match NaiveDateTime::parse_from_str(last_updated_str, "%Y-%m-%dT%H:%M:%S%.fZ") {
                        Ok(last_updated_date) => {
                            let saved_last_modified = get_playlist_last_modified(conn, &id)?;
                            println!("Saved last modified date: {:?}", saved_last_modified);
                            if let Some(saved_date) = saved_last_modified {
                                match NaiveDateTime::parse_from_str(&saved_date, "%Y-%m-%dT%H:%M:%S%.fZ") {
                                    Ok(saved_date_parsed) => {
                                        if saved_date_parsed >= last_updated_date {
                                            println!("[TIDAL] Skipping playlist: {}", name);
                                            // Fetch local version from the database
                                            let local_tracks = fetch_local_playlist_tracks(conn, &id)?;
                                            playlists.push(TidalPlaylist { id, name, tracks: local_tracks, last_updated });
                                            continue;
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("[TIDAL] Failed to parse saved date: {}", e);
                                        continue;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to parse last updated date: {}", e);
                            continue;
                        }
                    }
                }

                let mut items_url = playlist["relationships"]["items"]["links"]["self"].as_str().unwrap_or_default().to_string();
                let mut tracks = Vec::new();

                loop {
                    // Check if we have enough tokens, if not, wait
                    while remaining_tokens <= 1 {
                        sleep(Duration::from_secs(3)).await;
                        remaining_tokens = remaining_tokens + replenish_rate;
                    }

                    let items_response = Client::new()
                        .get(format!("{}{}", TIDAL_API, &items_url))
                        .bearer_auth(&client.token)
                        .send()
                        .await?;

                    if items_response.status().is_success() {
                        let items_response_headers = items_response.headers().clone();
                        let items_response_body = items_response.text().await?;
                        if items_response_body.is_empty() {
                            return Err("Empty items response body".into());
                        }

                        let items_response_json: Value = serde_json::from_str(&items_response_body)?;
                        let track_ids: Vec<String> = items_response_json["data"]
                            .as_array()
                            .unwrap_or(&vec![])
                            .iter()
                            .map(|item| item["id"].as_str().unwrap_or_default().to_string())
                            .collect();

                        // Fetch track details
                        let track_details = fetch_track_details(client, track_ids, "US").await?;
                        tracks.extend(track_details);
                        remaining_tokens -= get_requested_tokens(&items_response_headers);

                        if let Some(next_url) = items_response_json["links"]["next"].as_str() {
                            items_url = next_url.to_string();
                        } else {
                            break;
                        }
                    } else {
                        return Err(format!("Failed to fetch items: {}", items_response.status()).into());
                    }
                }

                playlists.push(TidalPlaylist { id, name, tracks, last_updated });
            }
        }

        Ok(playlists)
    } else {
        Err(format!("Failed to fetch playlists: {}", response.status()).into())
    }
}

fn fetch_local_playlist_tracks(conn: &Connection, playlist_id: &str) -> Result<Vec<TidalTrack>, Box<dyn std::error::Error>> {
    let mut stmt = conn.prepare("SELECT id, isrc FROM tracks WHERE playlist_id = ?1")?;
    let tracks = stmt.query_map(params![playlist_id], |row| {
        Ok(TidalTrack {
            id: row.get(0)?,
            attributes: TrackAttributes {
                title: String::new(), // Placeholder
                isrc: row.get(1)?,
                duration: String::new(), // Placeholder
                explicit: false, // Placeholder
                popularity: 0.0, // Placeholder
                availability: vec![], // Placeholder
                media_tags: vec![], // Placeholder
                external_links: vec![], // Placeholder
                copyright: String::new(), // Placeholder
            },
            relationships: TrackRelationships {
                albums: RelationshipLinks { links: RelationshipSelfLink { self_link: String::new() } },
                artists: RelationshipLinks { links: RelationshipSelfLink { self_link: String::new() } },
                providers: RelationshipLinks { links: RelationshipSelfLink { self_link: String::new() } },
                radio: RelationshipLinks { links: RelationshipSelfLink { self_link: String::new() } },
                similar_tracks: RelationshipLinks { links: RelationshipSelfLink { self_link: String::new() } },
            },
            links: TrackLinks { self_link: String::new() },
        })
    })?.collect::<Result<Vec<_>, _>>()?;

    Ok(tracks)
}

pub async fn fetch_track_details(client: &TidalClient, track_ids: Vec<String>, country_code: &str) -> Result<Vec<TidalTrack>, Box<dyn std::error::Error>> {
    let response = Client::new()
        .get(format!("{}/tracks", TIDAL_API))
        .query(&[("countryCode", country_code), ("filter[id]", &*track_ids.join(","))])
        .bearer_auth(&client.token)
        .send()
        .await?;

    if response.status().is_success() {
        let response_body = response.text().await?;
        if response_body.is_empty() {
            return Err("Empty response body".into());
        }

        let response_json: Value = serde_json::from_str(&response_body)?;
        let tracks: Vec<TidalTrack> = response_json["data"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|item| serde_json::from_value(item.clone()).unwrap())
            .collect();

        Ok(tracks)
    } else {
        Err(format!("Failed to fetch track details: {}", response.status()).into())
    }
}

fn get_remaining_tokens(headers: &HeaderMap) -> i32 {
    headers.get("X-RateLimit-Remaining")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn get_replenish_rate(headers: &HeaderMap) -> i32 {
    headers.get("X-RateLimit-Replenish-Rate")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .unwrap_or(1)
}

fn get_burst_capacity(headers: &HeaderMap) -> i32 {
    headers.get("X-RateLimit-Burst-Capacity")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .unwrap_or(1)
}

fn get_requested_tokens(headers: &HeaderMap) -> i32 {
    headers.get("X-RateLimit-Requested-Tokens")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .unwrap_or(1)
}