use oauth2::{
    AuthorizationCode, AuthUrl, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl,
    Scope, TokenResponse, TokenUrl, RefreshToken,
};
use oauth2::reqwest::async_http_client;
use oauth2::basic::BasicClient;
use crate::spotify::SpotifyClient;
use std::error::Error;
use std::fs::File;
use std::io::{self, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

fn store_tokens(access_token: &str, refresh_token: &str, expires_at: u64) -> io::Result<()> {
    let mut file = File::create("spotify_tokens.txt")?;
    file.write_all(format!("{}\n{}\n{}", access_token, refresh_token, expires_at).as_bytes())?;
    Ok(())
}

fn read_tokens() -> io::Result<(String, String, u64)> {
    let mut file = File::open("spotify_tokens.txt")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let tokens: Vec<&str> = contents.split('\n').collect();
    if tokens.len() == 3 {
        Ok((tokens[0].to_string(), tokens[1].to_string(), tokens[2].parse().unwrap()))
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid token file format"))
    }
}

fn is_token_expired(expires_at: u64) -> bool {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    now >= expires_at
}

pub async fn authenticate(config: &crate::config::Config) -> Result<SpotifyClient, Box<dyn Error>> {
    if let Ok((access_token, refresh_token, expires_at)) = read_tokens() {
        return if !is_token_expired(expires_at) {
            Ok(SpotifyClient::new(access_token))
        } else {
            let new_access_token = refresh_access_token(&refresh_token, config).await?;
            Ok(SpotifyClient::new(new_access_token))
        }
    }

    let client = BasicClient::new(
        ClientId::new(config.spotify.client_id.clone()),
        Some(ClientSecret::new(config.spotify.client_secret.clone())),
        AuthUrl::new("https://accounts.spotify.com/authorize".to_string())?,
        Some(TokenUrl::new("https://accounts.spotify.com/api/token".to_string())?),
    )
    .set_redirect_uri(RedirectUrl::new(config.spotify.redirect_uri.clone())?);

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("user-read-private".to_string()))
        .add_scope(Scope::new("user-read-email".to_string()))
        .add_scope(Scope::new("playlist-read-private".to_string()))
        .add_scope(Scope::new("playlist-read-collaborative".to_string()))
        .add_scope(Scope::new("playlist-modify-private".to_string()))
        .add_scope(Scope::new("playlist-modify-public".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    println!("Open this URL in your browser:\n{}", auth_url);
    print!("Enter the URL you were redirected to: ");
    io::stdout().flush()?;
    let mut input_url = String::new();
    io::stdin().read_line(&mut input_url)?;
    let auth_code = input_url.split("code=").collect::<Vec<&str>>()[1].split("&").collect::<Vec<&str>>()[0];

    let token_result = client
        .exchange_code(AuthorizationCode::new(auth_code.parse().unwrap()))
        .set_pkce_verifier(pkce_verifier)
        .request_async(async_http_client)
        .await?;

    let access_token = token_result.access_token().secret().to_string();
    let refresh_token = token_result.refresh_token().map(|token| token.secret().to_string()).unwrap_or_default();
    let expires_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + token_result.expires_in().unwrap().as_secs();

    store_tokens(&access_token, &refresh_token, expires_at)?;

    Ok(SpotifyClient::new(access_token))
}

pub async fn refresh_access_token(refresh_token: &str, config: &crate::config::Config) -> Result<String, Box<dyn Error>> {
    let client = BasicClient::new(
        ClientId::new(config.spotify.client_id.clone()),
        Some(ClientSecret::new(config.spotify.client_secret.clone())),
        AuthUrl::new("https://accounts.spotify.com/authorize".to_string())?,
        Some(TokenUrl::new("https://accounts.spotify.com/api/token".to_string())?),
    )
    .set_redirect_uri(RedirectUrl::new(config.spotify.redirect_uri.clone())?);

    let token_result = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token.to_string()))
        .request_async(async_http_client)
        .await?;

    let new_access_token = token_result.access_token().secret().to_string();
    let new_refresh_token = token_result.refresh_token().map(|t| t.secret().to_string()).unwrap_or(refresh_token.to_string());
    let expires_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + token_result.expires_in().unwrap().as_secs();

    store_tokens(&new_access_token, &new_refresh_token, expires_at)?;

    Ok(new_access_token)
}