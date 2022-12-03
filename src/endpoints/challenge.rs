use reqwest::Client;
use rocket::http::{Cookie, CookieJar, Status};
use rocket::State;
use crate::models::{Challenge, ChallengeAccept, LichessChallenge, LichessChallengeClock, LichessChallengeResponse, User};
use sqlx::Postgres;
use sqlx::Pool;

#[post("/api/challenge", data = "<challenge_request>")]
pub async fn challenge(user: User, pool: &State<Pool<Postgres>>, challenge_request: String) -> Result<String, Status> {
    println!("challenge request!: {}", challenge_request);
    let challenge_result: Result<Challenge, serde_json::Error> = serde_json::from_str(&challenge_request);
    let challenge = match challenge_result {
        Ok(c) => c,
        Err(e) => {
            println!("error: {}", e);
            return Err(Status::BadRequest)
        }
    };

    println!("challenge!{}", serde_json::to_string(&challenge).unwrap());

    // save challenge to db
    let status = "WAITING";
    let pg_query_result = sqlx::query_as::<_,Challenge>("INSERT INTO challenge (username, time_limit, opponent_time_limit, increment, color, sats, opp_username, status, expire_after) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *")
        .bind(&user.username)
        .bind(&challenge.time_limit)
        .bind(&challenge.opponent_time_limit)
        .bind(&challenge.increment)
        .bind(&challenge.color)
        .bind(&challenge.sats)
        .bind(&challenge.opp_username)
        .bind(status)
        .bind(&challenge.expire_after)
        .fetch_one(&**pool).await;

    return match pg_query_result {
        Ok(r) => {
            Ok(serde_json::to_string(&r).unwrap())
        },
        Err(e) => {
            println!("error: {}", e.as_database_error().unwrap().message());
            Err(Status::InternalServerError)
        }
    }
}

#[post("/api/challenge-accept", data = "<challenge_accept_request>")]
pub async fn challenge_accept(user: User, pool: &State<Pool<Postgres>>, challenge_accept_request: String) -> Result<String, Status> {
    let challenge_accept_result: Result<ChallengeAccept, serde_json::Error> = serde_json::from_str(&challenge_accept_request);
    let challenge_accept = match challenge_accept_result {
        Ok(c) => c,
        Err(e) => {
            println!("error: {}", e);
            return Err(Status::BadRequest)
        }
    };

    // look up challenge in db
    let challenge_result = sqlx::query_as::<_,Challenge>( "SELECT * FROM challenge WHERE id=$1 LIMIT 1")
        .bind(challenge_accept.id)
        .fetch_one(&**pool).await;
    let challenge = match challenge_result {
        Ok(c) => c,
        Err(e) => {
            println!("error: {}", e);
            return Err(Status::InternalServerError)
        }
    };

    if challenge.opp_username != user.username {
        println!("challenge can't be accepted by user.username: {}", user.username);
        return Err(Status::BadRequest)
    }

    // create on lichess
    let url = format!("https://lichess.org/api/challenge/{}", &challenge.username);
    let access_token = user.access_token;
    let bearer = format!("Bearer {access_token}");
    let body = parse_to_lichess_challenge(&challenge);
    println!("url: {}", url);
    println!("body: {}", serde_json::to_string(&body).unwrap());
    println!("bearer: {}", bearer);
    let resp = Client::new()
        .post(url)
        .json(&body)
        .header("Authorization", bearer)
        .send().await;

    let lichess_challenge_response: LichessChallengeResponse = match resp {
        Ok(res) => {
            println!("Status: {}", res.status());
            println!("Headers:\n{:#?}", res.headers());

            let text = res.text().await;
            match text {
                Ok(text) => {
                    println!("text!: {}", text);
                    serde_json::from_str(&text).unwrap()
                }
                Err(e) => {
                    println!("error: {}", e);
                    return Err(Status::InternalServerError)
                }
            }
        },
        Err(e) => {
            println!("error: {}", e);
            return Err(Status::InternalServerError)
        }
    };

    // update record in db
    let pg_query_result = sqlx::query("UPDATE challenge SET status='ACCEPTED', lichess_challenge_id=$1 WHERE id=$2")
        .bind(&lichess_challenge_response.challenge.id)
        .bind(&challenge.id)
        .execute(&**pool).await;

    // return url
    match pg_query_result {
        Ok(_) => Ok(serde_json::to_string(&lichess_challenge_response).unwrap()),
        Err(e) => {
            println!("error: {}", e);
            Err(Status::InternalServerError)
        }
    }
}

#[get("/api/challenges")]
pub async fn challenges(user: User, pool: &State<Pool<Postgres>>) -> Result<String, Status> {
    let challenges = sqlx::query_as::<_,Challenge>( "SELECT * FROM challenge WHERE username=$1 OR opp_username=$1 ORDER BY created_on DESC LIMIT 100")
        .bind(user.username)
        .fetch_all(&**pool).await;
    match challenges {
        Ok(challenges) => Ok(serde_json::to_string(&challenges).unwrap()),
        Err(e) => {
            println!("error: {}", e);
            Err(Status::InternalServerError)
        }
    }
}

fn parse_to_lichess_challenge(challenge: &Challenge) -> LichessChallenge {
    let color = match challenge.color.as_deref() {
        Some("white") => "black".to_string(),
        Some("black") => "white".to_string(),
        _ => "".to_string()
    };
    return LichessChallenge {
        rated: true,
        clock: LichessChallengeClock {
            limit: challenge.time_limit.unwrap_or(300).to_string(),
            increment: challenge.increment.unwrap_or(0).to_string(),
        },
        color,
        variant: "standard".to_string(),
        rules: "noClaimWin".to_string(),
    };
}