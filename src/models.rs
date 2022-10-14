use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Account {
    pub username: String
}

pub struct AppConfig {
    pub url: String,
    pub fe_url: String
}

#[derive(Serialize, Deserialize)]
pub struct Challenge {
    pub opponent: String,
    pub limit: u32, // seconds
    pub opponent_limit: u32, // seconds
    pub increment: u32, // seconds
    pub color: String,
    pub sats: u32,
}

#[derive(Serialize, Deserialize)]
pub struct LichessChallenge {
    pub rated: bool,
    pub clock: LichessChallengeClock,
    pub color: String,
    pub variant: String,
    pub rules: String,
}

#[derive(Serialize, Deserialize)]
pub struct LichessChallengeClock {
    pub limit: String,
    pub increment: String,
}

#[derive(Serialize, Deserialize)]
pub struct LichessChallengeResponse {
    pub challenge: Url
}

#[derive(Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String
}

#[derive(Serialize, Deserialize)]
pub struct Url {
    pub url: String
}

pub struct User {
    pub access_token: String,
    pub username: String,
}

#[derive(Serialize, Deserialize)]
pub struct UserProfile {
    pub username: String
}

