#[macro_use] extern crate rocket;

use std::env;
use cookie::SameSite;
use cookie::time::Duration;
use rand::{distributions::Alphanumeric, Rng};
use reqwest::Client;
use rocket::http::{Cookie, CookieJar};
use rocket::Request;
use rocket::request::{FromRequest, Outcome};
use rocket::response::Redirect;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use rocket::config::Config;

#[derive(Serialize, Deserialize)]
struct TokenResponse {
    access_token: String
}


#[derive(Serialize, Deserialize)]
struct Account {
    username: String
}

struct ProfileInfo {
    username: String
}
struct User {
    access_token: String,
    username: String,
}

#[get("/")]
async fn index(cookies: &CookieJar<'_>) -> &'static str {
    "hi"
}

#[get("/profile")]
async fn profile(user: User) -> String {
    user.username
}

#[get("/login")]
fn login(cookies: &CookieJar<'_>) -> Redirect {
    let redirect_uri = "http://localhost:8000/callback";

    // generate code verifier and challenge
    let rand: Vec<u8>  = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(128)
        .collect();
    let verifier = base64::encode_config(&rand, base64::URL_SAFE_NO_PAD);
    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = base64::encode_config(&digest, base64::URL_SAFE_NO_PAD);

    // add verifier to private cookie
    let cookie = Cookie::build("codeVerifier", verifier)
        .same_site(SameSite::None)
        .secure(true)
        .max_age(Duration::minutes(10))
        .finish();
    cookies.add_private(cookie);

    Redirect::to(format!("https://lichess.org/oauth?\
       response_type=code&\
       client_id=lightningchess&\
       redirect_uri={redirect_uri}&\
       scope=preference:read&\
       code_challenge_method=S256&\
       code_challenge={challenge}")
    )
}

#[get("/callback?<code>")]
async fn callback(code: String, cookies: &CookieJar<'_>) -> Option<Redirect> {
    let code_verifier: String = match cookies.get_private("codeVerifier") {
        Some(cookie) => {
            let cv = cookie.value().to_string();
            cookies.remove_private(cookie);
            cv
        }
        None => "".to_string()
    };

    let body = json!({
        "grant_type": "authorization_code",
        "redirect_uri": "http://localhost:8000/callback",
        "client_id": "lightningchess",
        "code": code,
        "code_verifier": code_verifier
    });
    println!("body: {}", body.to_string());

    return match Client::new()
        .post("https://lichess.org/api/token")
        .json(&body)
        .send().await {
        Ok(res) => {
            println!("Status: {}", res.status());
            println!("Headers:\n{:#?}", res.headers());

            match res.text().await {
                Ok(text) => {
                    let token_response: TokenResponse = serde_json::from_str(&text).unwrap();
                    let cookie = Cookie::build("access_token", token_response.access_token)
                        .same_site(SameSite::None)
                        .secure(true)
                        .max_age(Duration::days(365))
                        .finish();
                    cookies.add(cookie);
                }
                Err(e) => {
                    println!("error:\n{}", e);
                }
            };
            Some(Redirect::to(format!("http://localhost:8000")))
        },
        Err(e) => {
            println!("error:\n{}", e);
            None
        },
    }
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index, login, callback, profile])
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
    type Error = ();
    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let access_token = request.cookies().get("access_token").map(|c| c.value());
        match access_token {
            Some(token) => {
                let bearer = format!("Bearer {token}");
                let response = Client::new()
                    .get("https://lichess.org/api/account")
                    .header("Authorization", bearer)
                    .send().await;
                match response {
                    Ok(res) => {
                        println!("Status: {}", res.status());
                        println!("Headers:\n{:#?}", res.headers());
                        let text = res.text().await;
                        match text {
                            Ok(text) => {
                                let account: Account = serde_json::from_str(&text).unwrap();
                                Outcome::Success(User { access_token: token.to_string(), username: account.username})
                            }
                            Err(e) => {
                                println!("error in text():\n{}", e);
                                Outcome::Forward(())
                            }
                        }
                    },
                    Err(e) => {
                        println!("error from api/account:\n{}", e);
                        Outcome::Forward(())
                    }
                }
            }
            None => {
                println!("no access token\n");
                Outcome::Forward(())
            }
        }
    }
}
