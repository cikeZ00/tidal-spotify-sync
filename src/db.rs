use rusqlite::{params, Connection, OptionalExtension, Result};
use crate::spotify::data::fetch_spotify_playlist;
use crate::spotify::SpotifyClient;

pub fn initialize_db() -> Result<Connection> {
    let conn = Connection::open("playlists.db")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS playlists (
            id INTEGER PRIMARY KEY,
            tidal_id TEXT NOT NULL,
            spotify_id TEXT NOT NULL,
            name TEXT NOT NULL,
            last_modified TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tracks (
            id INTEGER PRIMARY KEY,
            playlist_id INTEGER NOT NULL,
            isrc TEXT NOT NULL,
            FOREIGN KEY(playlist_id) REFERENCES playlists(id)
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS blacklist (
            isrc TEXT PRIMARY KEY
        )",
        [],
    )?;
    Ok(conn)
}

pub fn insert_blacklist(conn: &Connection, isrc: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO blacklist (isrc) VALUES (?1)",
        params![isrc],
    )?;
    Ok(())
}

pub fn is_blacklisted(conn: &Connection, isrc: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM blacklist WHERE isrc = ?1",
        params![isrc],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn insert_playlist(conn: &Connection, tidal_id: &str, spotify_id: &str, name: &str, last_modified: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO playlists (tidal_id, spotify_id, name, last_modified) VALUES (?1, ?2, ?3, ?4)",
        params![tidal_id, spotify_id, name, last_modified],
    )?;
    Ok(())
}

pub fn insert_track(conn: &Connection, playlist_id: i64, isrc: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO tracks (playlist_id, isrc) VALUES (?1, ?2)",
        params![playlist_id, isrc],
    )?;
    Ok(())
}

pub fn get_playlist_id(conn: &Connection, tidal_id: &str) -> Result<i64> {
    conn.query_row(
        "SELECT id FROM playlists WHERE tidal_id = ?1",
        params![tidal_id],
        |row| row.get(0),
    )
}

pub fn get_spotify_playlist_id(conn: &Connection, tidal_id: &str) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let mut stmt = conn.prepare("SELECT spotify_id FROM playlists WHERE tidal_id = ?1")?;
    let spotify_id: String = stmt.query_row(params![tidal_id], |row| row.get(0))?;
    Ok(spotify_id)
}

pub fn get_playlist_last_modified(conn: &Connection, tidal_id: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT last_modified FROM playlists WHERE tidal_id = ?1",
        params![tidal_id],
        |row| row.get(0),
    ).optional()
}

pub async fn check_playlist_integrity(conn: &Connection, spotify_client: &SpotifyClient, playlist_id: i64) -> Result<bool> {
    let mut stmt = conn.prepare("SELECT isrc FROM tracks WHERE playlist_id = ?1")?;
    let track_isrcs: Vec<String> = stmt.query_map(params![playlist_id], |row| row.get::<_, String>(0))?
        .map(|res| res.unwrap())
        .filter(|isrc| !is_blacklisted(conn, isrc).unwrap_or(false))
        .collect();

    let spotify_playlist_id: String = conn.query_row(
        "SELECT spotify_id FROM playlists WHERE id = ?1",
        params![playlist_id],
        |row| row.get(0),
    )?;

    let spotify_tracks = fetch_spotify_playlist(spotify_client, &spotify_playlist_id).await.expect("Failed to fetch Spotify playlist");
    let spotify_isrcs: Vec<String> = spotify_tracks["tracks"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["track"]["external_ids"]["isrc"].as_str().unwrap().to_string())
        .collect();

    Ok(track_isrcs == spotify_isrcs)
}

pub fn playlist_exists(conn: &Connection, tidal_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM playlists WHERE tidal_id = ?1",
        params![tidal_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}