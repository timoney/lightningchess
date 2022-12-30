use reqwest::Client;
use rocket::http::{Status};
use rocket::State;
use crate::models::{Balance, Challenge, ChallengeAcceptRequest, LichessChallenge, LichessChallengeClock, LichessChallengeResponse, Transaction, User};
use sqlx::Postgres;
use sqlx::Pool;

#[post("/api/challenge", data = "<challenge_request>")]
pub async fn create_challenge(user: User, pool: &State<Pool<Postgres>>, challenge_request: String) -> Result<String, Status> {
    println!("challenge request!: {}", challenge_request);
    let challenge_result: Result<Challenge, serde_json::Error> = serde_json::from_str(&challenge_request);
    let challenge = match challenge_result {
        Ok(c) => c,
        Err(e) => {
            println!("error: {}", e);
            return Err(Status::BadRequest)
        }
    };

    // only allow creation of challenge if user has enough funds
    let balance_result = sqlx::query_as::<_,Balance>( "SELECT * FROM lightningchess_balance WHERE username=$1")
        .bind(&user.username)
        .fetch_one(&**pool).await;
    match balance_result {
        Ok(balance) => {
            if balance.balance < 0 || balance.balance < challenge.sats.unwrap() {
                return Err(Status::PaymentRequired)
            }
        },
        Err(e) => {
            println!("error: {}", e);
            return Err(Status::InternalServerError)
        }
    }

    //create transaction
    let tx_result = pool.begin().await;
    let mut tx = match tx_result {
        Ok(t) => t,
        Err(e) => {
            println!("error: {}", e);
            return Err(Status::InternalServerError)
        }
    };

    // save challenge to db
    let status = "WAITING FOR ACCEPTANCE";
    let challenge_result = sqlx::query_as::<_,Challenge>("INSERT INTO challenge (username, time_limit, opponent_time_limit, increment, color, sats, opp_username, status, expire_after) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *")
        .bind(&user.username)
        .bind(&challenge.time_limit)
        .bind(&challenge.opponent_time_limit)
        .bind(&challenge.increment)
        .bind(&challenge.color)
        .bind(&challenge.sats)
        .bind(&challenge.opp_username)
        .bind(&status)
        .bind(&challenge.expire_after)
        .fetch_one(&mut tx).await;

    let challenge_json_result = match challenge_result {
        Ok(r) => {
            Ok(serde_json::to_string(&r).unwrap())
        },
        Err(e) => {
            println!("insert challenge error: {}", e);
            return Err(Status::InternalServerError)
        }
    };

    // deduct from balance
    let balance_result = sqlx::query_as::<_,Balance>( "UPDATE lightningchess_balance SET balance=balance - $1 WHERE username=$2 RETURNING * ")
        .bind(challenge.sats.unwrap())
        .bind(&user.username)
        .fetch_one(&mut tx).await;

    match balance_result {
        Ok(balance) => {
            // just another check
            if balance.balance < 0 {
                println!("balance is less than 0");
                return Err(Status::InternalServerError)
            }
            println!("updated balance")
        },
        Err(e) => {
            println!("error updating balance: {}", e);
            return Err(Status::InternalServerError)
        }
    }

    // insert transaction into transaction db
    let ttype = "create challenge";
    let detail = format!("challenge vs {}", challenge.opp_username);
    let state = "SETTLED";
    let transaction_result = sqlx::query_as::<_,Transaction>("INSERT INTO lightningchess_transaction (username, ttype, detail, amount, state) VALUES ($1, $2, $3, $4, $5) RETURNING *")
        .bind(&user.username)
        .bind(ttype)
        .bind(&detail)
        .bind(-challenge.sats.unwrap())
        .bind(state)
        .fetch_one(&mut tx).await;

    match transaction_result {
        Ok(_) => println!("successfully inserted transaction"),
        Err(e) => {
            println!("error inserting transaction: {}", e);
            return Err(Status::InternalServerError)
        }
    }

    let commit_result = tx.commit().await;
    return match commit_result {
        Ok(_) => {
            challenge_json_result
        },
        Err(e) => {
            println!("error committing: {}", e);
            Err(Status::InternalServerError)
        }
    }
}

#[post("/api/accept-challenge", data = "<challenge_accept_request>")]
pub async fn accept_challenge(user: User, pool: &State<Pool<Postgres>>, challenge_accept_request: String) -> Result<String, Status> {
    println!("challenge_accept_request!: {}", challenge_accept_request);
    let challenge_accept_request_result: Result<ChallengeAcceptRequest, serde_json::Error> = serde_json::from_str(&challenge_accept_request);
    let challenge_accept_request = match challenge_accept_request_result {
        Ok(c) => c,
        Err(e) => {
            println!("error: {}", e);
            return Err(Status::BadRequest)
        }
    };

    let challenge_result = sqlx::query_as::<_,Challenge>( "SELECT * FROM challenge WHERE id=$1")
        .bind(challenge_accept_request.id)
        .fetch_one(&**pool).await;

    let challenge = match challenge_result {
        Ok(c) => c,
        Err(e) => {
            println!("error getting challenge in challenge accept: {}", e.as_database_error().unwrap().message());
            return Err(Status::InternalServerError)
        }
    };

    // only opponent can accept the challenge and challenge must be in correct status
    if challenge.opp_username != user.username || challenge.status.as_ref().unwrap() != "WAITING FOR ACCEPTANCE" {
        return Err(Status::BadRequest)
    }

    // only allow accept of challenge if user has enough funds
    let balance_result = sqlx::query_as::<_,Balance>( "SELECT * FROM lightningchess_balance WHERE username=$1")
        .bind(&user.username)
        .fetch_one(&**pool).await;
    match balance_result {
        Ok(balance) => {
            if balance.balance < 0 || balance.balance < challenge.sats.unwrap() {
                return Err(Status::PaymentRequired)
            }
        },
        Err(e) => {
            println!("error: {}", e);
            return Err(Status::InternalServerError)
        }
    }

    //create transaction
    let tx_result = pool.begin().await;
    let mut tx = match tx_result {
        Ok(t) => t,
        Err(e) => {
            println!("error creating tx: {}", e);
            return Err(Status::InternalServerError)
        }
    };

    // deduct balance
    let balance_result = sqlx::query_as::<_,Balance>( "UPDATE lightningchess_balance SET balance=balance - $1 WHERE username=$2 RETURNING *")
        .bind(challenge.sats.unwrap())
        .bind(&user.username)
        .fetch_one(&mut tx).await;

    match balance_result {
        Ok(balance) => {
            if balance.balance < 0 {
                println!("balance is less than 0");
                return Err(Status::InternalServerError)
            }
            println!("updated balance")
        },
        Err(e) => {
            println!("error updating balance: {}", e);
            return Err(Status::InternalServerError)
        }
    };

    // insert transaction into transaction db
    let ttype = "accept challenge";
    let detail = format!("challenge vs {}", challenge.username);
    let state = "SETTLED";
    let transaction_result = sqlx::query_as::<_,Transaction>("INSERT INTO lightningchess_transaction (username, ttype, detail, amount, state) VALUES ($1, $2, $3, $4, $5) RETURNING *")
        .bind(&user.username)
        .bind(ttype)
        .bind(&detail)
        .bind(-challenge.sats.unwrap())
        .bind(state)
        .fetch_one(&mut tx).await;

    match transaction_result {
        Ok(_) => println!("successfully inserted transaction"),
        Err(e) => {
            println!("error inserting tx: {}", e);
            return Err(Status::InternalServerError)
        }
    }

    // create on lichess
    let url = format!("https://lichess.org/api/challenge/{}", &challenge.username);
    let access_token = user.access_token;
    let bearer = format!("Bearer {access_token}");
    let body = parse_to_lichess_challenge(&challenge);
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
            println!("error creating on lichess: {}", e);
            return Err(Status::InternalServerError)
        }
    };

    // update challenge in db
    let status = "ACCEPTED";
    let pg_query_result = sqlx::query_as::<_,Challenge>("UPDATE challenge SET status=$1, lichess_challenge_id=$2 WHERE id=$3 RETURNING *")
        .bind(status)
        .bind(&lichess_challenge_response.challenge.id)
        .bind(challenge_accept_request.id)
        .fetch_one(&mut tx).await;

    let challenge_json_result = match pg_query_result {
        Ok(r) => {
            Ok(serde_json::to_string(&r).unwrap())
        },
        Err(e) => {
            println!("update challenge in challenge accept: {}", e);
            return Err(Status::InternalServerError)
        }
    };

    // commit transaction, return challenge
    let commit_result = tx.commit().await;
    return match commit_result {
        Ok(_) => {
            challenge_json_result
        },
        Err(e) => {
            println!("error committing: {}", e);
            Err(Status::InternalServerError)
        }
    }

}
#[get("/api/challenges")]
pub async fn challenges(user: User, pool: &State<Pool<Postgres>>) -> Result<String, Status> {
    let challenges = sqlx::query_as::<_,Challenge>( "SELECT * FROM challenge WHERE username=$1 OR opp_username=$1 ORDER BY created_on DESC LIMIT 100")
        .bind(&user.username)
        .fetch_all(&**pool).await;
    match challenges {
        Ok(challenges) => Ok(serde_json::to_string(&challenges).unwrap()),
        Err(e) => {
            println!("error: {}", e);
            Err(Status::InternalServerError)
        }
    }
}

#[get("/api/challenge/<challenge_id>")]
pub async fn lookup_challenge(user: User, pool: &State<Pool<Postgres>>, challenge_id: String) -> Result<String, Status> {
    let challenge_id_int = match challenge_id.parse::<i32>() {
        Ok(i) => i,
        Err(_) => return Err(Status::BadRequest)
    };
    let challenge = sqlx::query_as::<_,Challenge>( "SELECT * FROM challenge WHERE id=$1")
        .bind(challenge_id_int)
        .fetch_one(&**pool).await;

    match challenge {
        Ok(challenge) =>  {
            // only be able to look up own games
            if challenge.username != user.username && challenge.opp_username != user.username {
                Err(Status::Unauthorized)
            } else {
                Ok(serde_json::to_string(&challenge).unwrap())
            }
        },
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