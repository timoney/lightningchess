#[macro_use] extern crate rocket;

use std::collections::HashMap;
use cookie::SameSite;
use cookie::time::Duration;
use rand::{distributions::Alphanumeric, Rng};
use reqwest::Client;
use rocket::http::{Cookie, CookieJar};
use rocket::{Request, State};
use rocket::fairing::AdHoc;
use rocket::figment::Provider;
use rocket::request::{FromRequest, Outcome};
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

pub mod guard;

#[derive(Serialize, Deserialize)]
struct TokenResponse {
    access_token: String
}

#[derive(Serialize, Deserialize)]
struct Account {
    username: String
}

#[derive(Serialize, Deserialize)]
struct UserProfile {
    username: String
}
struct User {
    access_token: String,
    username: String,
}

struct AppConfig {
    url: String,
    fe_url: String
}

#[derive(Serialize, Deserialize)]
struct Challenge {
    opponent: String,
    limit: String,
    opponent_limit: String,
    increment: String,
    challenger_color: String,
    sats: String,
}

#[derive(Serialize, Deserialize)]
struct LichessChallengeClock {
    limit: String,
    increment: String,
}

#[derive(Serialize, Deserialize)]
struct LichessChallenge {
    rated: bool,
    clock: LichessChallengeClock,
    color: String,
    variant: String,
    rules: String,
}
#[derive(Serialize, Deserialize)]
struct Url {
    url: String
}

#[derive(Serialize, Deserialize)]
struct LichessChallengeResponse {
    challenge: Url
}

#[get("/")]
fn index(app_config: &State<AppConfig>,) -> Template {
    let mut context = HashMap::new();
    context.insert("fe_url", app_config.fe_url.to_string());
    Template::render("index", &context)
}

#[get("/profile")]
async fn profile(user: User) -> String {
    let userProfile: UserProfile = UserProfile {
        username: user.username
    };
    serde_json::to_string(&userProfile).unwrap()
}

#[post("/api/challenge", data = "<challenge_request>")]
async fn challenge(challenge_request: String, cookies: &CookieJar<'_>) -> String {
    println!("challenge request!: {}", challenge_request);
    let challenge: Challenge = serde_json::from_str(&challenge_request).unwrap();
    println!("challenge!{}", serde_json::to_string(&challenge).unwrap());
    let url = format!("https://lichess.org/api/challenge/{}", &challenge.opponent);
    let access_token = cookies.get("access_token").map(|c| c.value()).unwrap();
    let bearer = format!("Bearer {access_token}");
    let body = LichessChallenge {
        rated: true,
        clock: LichessChallengeClock {
            limit: challenge.limit,
            increment: challenge.increment,
        },
        color: challenge.challenger_color,
        variant: "standard".to_string(),
        rules: "noClaimWin".to_string(),
    };
    println!("url: {}", url);
    println!("body: {}", serde_json::to_string(&body).unwrap());
    println!("bearer: {}", bearer);
    let resp = Client::new()
        .post(url)
        .json(&body)
        .header("Authorization", bearer)
        .send().await;

    return match resp {
        Ok(res) => {
            println!("Status: {}", res.status());
            println!("Headers:\n{:#?}", res.headers());

            let text = res.text().await;
            match text {
                Ok(text) => {
                    println!("text!: {}", text);
                    let resp: LichessChallengeResponse = serde_json::from_str(&text).unwrap();
                    serde_json::to_string(&resp).unwrap()
                }
                Err(e) => {
                    println!("error:\n{}", e);
                    "error:(".to_string()
                }
            }
        },
        Err(error) => {
            println!("error:\n{}", error);
            "error:(:(".to_string()
        }
    }
}

#[get("/login")]
fn login(app_config: &State<AppConfig>, cookies: &CookieJar<'_>) -> Redirect {
    let redirect_uri = format!("{}/callback", &app_config.url);

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
       scope=preference:read%20challenge:write&\
       code_challenge_method=S256&\
       code_challenge={challenge}")
    )
}

#[get("/callback?<code>")]
async fn callback(code: String, app_config: &State<AppConfig>, cookies: &CookieJar<'_>) -> Option<Redirect> {
    let redirect_uri = format!("{}/callback", &app_config.url);
    let code_verifier: String = match cookies.get_private("codeVerifier") {
        Some(cookie) => {
            let cv = cookie.value().to_string();
            cookies.remove_private(cookie);
            cv
        }
        None => {
            println!("No code verifier found!");
            "".to_string()
        }
    };

    let body = json!({
        "grant_type": "authorization_code",
        "redirect_uri": redirect_uri,
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
                    println!("text!: {}", text);
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
            Some(Redirect::to(format!("{}", &app_config.url)))
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
        .attach(AdHoc::try_on_ignite("appConfig", |rocket| async {
            match rocket.figment().data() {
                Ok(map) => {
                    for (k, v) in map.iter() {
                        println!("key {}", k);
                        for (k2, v2) in v.into_iter() {
                            println!("key: {}", k2);
                        }
                    }
                },
                Err(e) => {
                    info!("error data: {e}");
                }
            }
            let fe_url: String = match rocket.figment().extract_inner::<String>("fe_url") {
                Ok(value) => {
                    info!("fe url: {value}");
                    value
                },
                Err(e) => {
                    info!("error: {e}");
                    "".to_string()
                }
            };

            match rocket.figment().extract_inner("url") {
                Ok(value) => {
                    info!("api host: {value}");
                    Ok(rocket.manage(AppConfig { url: value, fe_url } ))
                },
                Err(e) => {
                    info!("error: {e}");
                    Err(rocket)
                }
            }
        }))
        .mount("/", routes![index, login, callback, profile, challenge])
        .attach(Template::fairing())
}
