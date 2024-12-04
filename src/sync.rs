use rusqlite::params;
use chrono::NaiveDateTime;
use crate::db::{initialize_db, insert_playlist, insert_track, get_playlist_id, check_playlist_integrity, playlist_exists, get_spotify_playlist_id};
use crate::tidal::data::fetch_playlists;
use crate::spotify::data::{create_playlist, add_tracks_to_playlist, fetch_spotify_playlist};
use crate::tidal::TidalClient;
use crate::spotify::SpotifyClient;

pub async fn sync_data(
    tidal_client: &TidalClient,
    spotify_client: &SpotifyClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = initialize_db()?;
    let tidal_playlists = fetch_playlists(tidal_client, &conn).await?;

    for playlist in tidal_playlists {
        let last_modified = playlist.last_updated.as_deref().unwrap_or("");

        if playlist_exists(&conn, &playlist.id)? {
            let playlist_id = get_playlist_id(&conn, &playlist.id)?;

            // Run integrity check and sync tracks
            let integrity_ok = check_playlist_integrity(&conn, spotify_client, playlist_id).await?;
            if !integrity_ok {
                eprintln!("Integrity check failed for playlist: {}", playlist.name);
            }

            // Fetch Spotify playlist and check for missing tracks
            let created_playlist_id = get_spotify_playlist_id(&conn, &playlist.id)?;
            let spotify_tracks = fetch_spotify_playlist(spotify_client, &created_playlist_id).await?;
            let spotify_isrcs: Vec<String> = spotify_tracks["tracks"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["track"]["external_ids"]["isrc"].as_str().unwrap().to_string())
                .collect();

            let mut stmt = conn.prepare("SELECT isrc FROM tracks WHERE playlist_id = ?1")?;
            let local_isrcs: Vec<String> = stmt.query_map(params![playlist_id], |row| row.get(0))?
                .map(|res| res.unwrap())
                .collect();

            let missing_isrcs: Vec<String> = local_isrcs.into_iter().filter(|isrc| !spotify_isrcs.contains(isrc)).collect();
            if !missing_isrcs.is_empty() {
                add_tracks_to_playlist(spotify_client, &created_playlist_id, missing_isrcs, &conn).await?;
            }
        } else {
            let created_playlist_id = create_playlist(spotify_client, &format!("{} [TIDAL]", &playlist.name), "Automatically synced Tidal playlist", true).await?;
            insert_playlist(&conn, &playlist.id, &created_playlist_id, &playlist.name, last_modified)?;

            let playlist_id = get_playlist_id(&conn, &playlist.id)?;
            for track in &playlist.tracks {
                insert_track(&conn, playlist_id, &track.attributes.isrc)?;
            }

            let integrity_ok = check_playlist_integrity(&conn, spotify_client, playlist_id).await?;
            if !integrity_ok {
                eprintln!("Integrity check failed for playlist: {}", playlist.name);
            }

            // Fetch Spotify playlist and check for missing tracks
            let spotify_tracks = fetch_spotify_playlist(spotify_client, &created_playlist_id).await?;
            let spotify_isrcs: Vec<String> = spotify_tracks["tracks"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["track"]["external_ids"]["isrc"].as_str().unwrap().to_string())
                .collect();

            let mut stmt = conn.prepare("SELECT isrc FROM tracks WHERE playlist_id = ?1")?;
            let local_isrcs: Vec<String> = stmt.query_map(params![playlist_id], |row| row.get(0))?
                .map(|res| res.unwrap())
                .collect();

            let missing_isrcs: Vec<String> = local_isrcs.into_iter().filter(|isrc| !spotify_isrcs.contains(isrc)).collect();
            if !missing_isrcs.is_empty() {
                add_tracks_to_playlist(spotify_client, &created_playlist_id, missing_isrcs, &conn).await?;
            }
        }
    }

    Ok(())
}