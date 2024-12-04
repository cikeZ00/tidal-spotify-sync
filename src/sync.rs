use crate::tidal::data::{fetch_playlists};
use crate::spotify::data::{create_playlist, add_tracks_to_playlist};
use crate::tidal::TidalClient;
use crate::spotify::SpotifyClient;

// TODO: Implement a local database to store the mapping between Tidal and Spotify tracks
// We can use the Tidal last modified date to determine if a playlist has been updated
// We should also skip tracks that are already in the Spotify playlist


pub async fn sync_data(
    tidal_client: &TidalClient,
    spotify_client: &SpotifyClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let tidal_playlists = fetch_playlists(tidal_client).await?;
    
    for playlist in tidal_playlists {
        let created_playlist_id = create_playlist(spotify_client, &playlist.name, "Automatically synced Tidal playlist", true).await?;
        let tracks = playlist.tracks.iter().map(|track| track.attributes.isrc.clone()).collect();
        add_tracks_to_playlist(spotify_client, &created_playlist_id, tracks).await?;
    }
    
    Ok(())
}
