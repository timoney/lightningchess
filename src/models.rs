use serde::{Deserialize, Serialize};
use sqlx::types::chrono::NaiveDateTime;
use sqlx::FromRow;

#[derive(Serialize, Deserialize)]
pub struct Account {
    pub username: String
}

pub struct AppConfig {
    pub url: String,
    pub fe_url: String
}

pub struct EnvVariables {
    pub macaroon: String // hex encoded
}

fn default_string() -> String {
    "".to_string()
}
fn default_i32() -> i32 {
    0
}

#[derive(Serialize, Deserialize, FromRow)]
pub struct Challenge {
    #[serde(default = "default_i32")]
    pub id: i32,
    #[serde(default = "default_string")]
    pub username: String,
    pub time_limit: Option<i32>, // seconds
    pub opponent_time_limit: Option<i32>, // seconds
    pub increment: Option<i32>, // seconds
    pub color: Option<String>,
    pub sats: Option<i64>,
    pub opp_username: String,
    pub status: Option<String>,
    pub lichess_challenge_id: Option<String>,
    pub result: Option<String>,
    pub created_on: Option<NaiveDateTime>, // UTC
    pub expire_after: Option<i32>, // seconds,
    pub payment_addr: Option<String>,
    pub payment_request: Option<String>,
    pub opp_payment_addr: Option<String>,
    pub opp_payment_request: Option<String>
}

#[derive(Serialize, Deserialize)]
pub struct ChallengeAccept {
    pub id: i32,
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
pub struct LichessChallengeAcceptResponse {
    pub ok: bool
}

#[derive(Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String
}

#[derive(Serialize, Deserialize)]
pub struct Url {
    pub id: String,
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

// LND
#[derive(Serialize, Deserialize)]
pub struct AddHoldInvoiceResponse {
    pub payment_request: String,
    pub add_index: String,
    pub payment_addr: String
}
