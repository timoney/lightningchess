use std::env;
use rand::distributions::Alphanumeric;
use rand::Rng;
use reqwest::Client;
use rocket::http::Status;
use rocket::State;
use sqlx::{Pool, Postgres};
use crate::models::{Transaction, AddInvoiceRequest, User, Balance, Challenge, LichessExportGameResponse, SendPaymentRequest, SendPaymentResponse};
use crate::lightning::invoices::add_invoice;
use crate::lightning::payment::{decode_payment, make_payment};

#[post("/api/invoice", data = "<invoice_request_str>")]
pub async fn add_invoice_endpoint(user: User, pool: &State<Pool<Postgres>>, invoice_request_str: String) -> Result<String, Status> {
    println!("invoice request: {}", invoice_request_str);
    let invoice_request_result: Result<AddInvoiceRequest, serde_json::Error> = serde_json::from_str(&invoice_request_str);
    let invoice_request = match invoice_request_result {
        Ok(i) => i,
        Err(e) => {
            println!("error: {}", e);
            return Err(Status::BadRequest)
        }
    };

    // create preimage
    let preimage_bytes: Vec<u8>  = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .collect();

    let preimage =  base64::encode(&preimage_bytes);
    // create memo
    let memo = format!("funding account {} on lightningchess.io", &user.username);

    // create invoice
    let add_invoice_response_option = add_invoice(invoice_request.sats, &memo, preimage_bytes).await;
    let add_invoice_response = match add_invoice_response_option {
        Some(i) => i,
        None => return Err(Status::InternalServerError)
    };

    // save it to db
    let ttype = "invoice";
    let state = "OPEN";
    // TODO: change to return without the preimage
    let pg_query_result = sqlx::query_as::<_,Transaction>("INSERT INTO lightningchess_transaction (username, ttype, detail, amount, state, preimage, payment_addr, payment_request) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *")
        .bind(&user.username)
        .bind(ttype)
        .bind(&memo)
        .bind(0) // default to zero until paid
        .bind(state)
        .bind(&preimage)
        .bind(&add_invoice_response.payment_addr)
        .bind(&add_invoice_response.payment_request)
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

#[post("/api/transaction/<transaction_id>")]
pub async fn lookup_transaction(user: User, pool: &State<Pool<Postgres>>, transaction_id: String) -> Result<String, Status> {
    let transaction_id_int = match transaction_id.parse::<i32>() {
        Ok(i) => i,
        Err(_) => return Err(Status::BadRequest)
    };

    let transaction_result = sqlx::query_as::<_,Transaction>( "SELECT * FROM lightningchess_transaction WHERE transaction_id=$1")
        .bind(transaction_id_int)
        .fetch_one(&**pool).await;

    let transaction = match transaction_result {
        Ok(t) => t,
        Err(e) => {
            println!("error: {}", e);
            return Err(Status::InternalServerError)
        }
    };

    if transaction.username != user.username {
        return Err(Status::Unauthorized)
    }

    let transaction_result2 = sqlx::query_as::<_,Transaction>( "SELECT * FROM lightningchess_transaction WHERE transaction_id=$1")
        .bind(transaction_id_int)
        .fetch_one(&**pool).await;

    return match transaction_result2 {
        Ok(t2) => Ok(serde_json::to_string(&t2).unwrap()),
        Err(e) => {
            println!("error getting t2: {}", e);
            return Err(Status::InternalServerError)
        }
    }
}

#[get("/api/transactions")]
pub async fn transactions(user: User, pool: &State<Pool<Postgres>>) -> Result<String, Status> {
    let transactions = sqlx::query_as::<_,Transaction>( "SELECT * FROM lightningchess_transaction WHERE username=$1 ORDER BY transaction_id DESC LIMIT 100")
        .bind(&user.username)
        .fetch_all(&**pool).await;

    match transactions {
        Ok(t) => Ok(serde_json::to_string(&t).unwrap()),
        Err(e) => {
            println!("error: {}", e);
            Err(Status::InternalServerError)
        }
    }
}

#[get("/api/balance")]
pub async fn balance(user: User, pool: &State<Pool<Postgres>>) -> Result<String, Status> {

    check_pending_invoices_and_update(&user, pool, None).await;
    check_pending_challenges_and_update(&user, pool).await;

    let balance_result = sqlx::query_as::<_,Balance>( "SELECT * FROM lightningchess_balance WHERE username=$1")
        .bind(&user.username)
        .fetch_optional(&**pool).await;
    match balance_result {
        Ok(balance_option) => {
            match balance_option {
                Some(balance) => Ok(serde_json::to_string(&balance).unwrap()),
                None => Ok(serde_json::to_string(&Balance {
                    balance_id: 0,
                    username: user.username,
                    balance: 0
                }).unwrap())
            }
        },
        Err(e) => {
            println!("error: {}", e);
            Err(Status::InternalServerError)
        }
    }
}

async fn check_pending_invoices_and_update(user: &User, pool: &State<Pool<Postgres>>, transaction_id: Option<i32>) -> () {
    // 1. look up all transactions in db for a user where the type is invoice and status is OPEN
    let transactions_result = match transaction_id {
        Some(tid) => {
            sqlx::query_as::<_, Transaction>("SELECT * FROM lightningchess_transaction WHERE transaction_id=$1 AND state='OPEN'")
                .bind(tid)
                .fetch_all(&**pool).await
        },
        None => {
            sqlx::query_as::<_, Transaction>("SELECT * FROM lightningchess_transaction WHERE username=$1 AND state='OPEN' AND ttype='invoice' ORDER BY transaction_id DESC LIMIT 100")
                .bind(&user.username)
                .fetch_all(&**pool).await
        }
    };

    let transactions = match transactions_result {
        Ok(ts) => ts,
        Err(e) => return println!("unable to fetch transactions: {}", e)
    };

    // 2. TODO: parallelize this
    for transaction in transactions.iter() {
        println!("processing transaction {}", transaction.transaction_id);
        let invoice_option = crate::lightning::hodl_invoices::lookup_hodl_invoice(transaction.payment_addr.as_ref().unwrap()).await;
        match invoice_option {
            Some(i) => {
                let new_state = i.state;
                // 3. update if necessary in postgres
                if new_state != "OPEN" {
                    let tx_result = pool.begin().await;
                    let mut tx = match tx_result {
                        Ok(t) => t,
                        Err(e) => {
                            println!("error creating tx: {}", e);
                            return;
                        }
                    };
                    // update transaction table
                    let amount = i.amt_paid_sat.parse::<i64>().unwrap();
                    let updated_transaction = sqlx::query( "UPDATE lightningchess_transaction SET state=$1, amount=$2 WHERE transaction_id=$3")
                        .bind(&new_state)
                        .bind(amount)
                        .bind(transaction.transaction_id)
                        .execute(&mut tx).await;

                    match updated_transaction {
                        Ok(_) => println!("successfully updated_transaction transaction id {}", transaction.transaction_id),
                        Err(e) => {
                            println!("error updated_transaction transaction id : {}, {}", transaction.transaction_id, e);
                            return;
                        }
                    }

                    // update balance table
                    if new_state == "SETTLED" {
                        //"INSERT INTO lightningchess_balance (username, balance) VALUES ($1, $2) ON CONFLICT DO UPDATE lightningchess_balance SET balance=(balance + $3) WHERE username=$4"
                        let updated_balance = sqlx::query( "INSERT INTO lightningchess_balance (username, balance) VALUES ($1, $2) ON CONFLICT (username) DO UPDATE SET balance=lightningchess_balance.balance + $3 WHERE lightningchess_balance.username=$4")
                            .bind(&user.username)
                            .bind(amount)
                            .bind(amount)
                            .bind(&user.username)
                            .execute(&mut tx).await;

                        match updated_balance {
                            Ok(_) => println!("successfully updated_balance transaction id {}", transaction.transaction_id),
                            Err(e) => {
                                println!("error updated_balance transaction id : {}, {}", transaction.transaction_id, e);
                                return;
                            }
                        }
                    }

                    // commit
                    let commit_result = tx.commit().await;
                    match commit_result {
                        Ok(_) => println!("successfully committed transaction id {}", transaction.transaction_id),
                        Err(_) => {
                            println!("error committing transaction id : {}", transaction.transaction_id);
                            return;
                        }
                    }
                }
            },
            None => println!("lookup_hodl_invoice error for tx id : {}", transaction.transaction_id)
        };
    }
}

async fn check_pending_challenges_and_update(user: &User, pool: &State<Pool<Postgres>>) -> () {
    // 1. look up all the challenges in ACCEPTED status
    let challenges_result = sqlx::query_as::<_,Challenge>( "SELECT * FROM challenge WHERE (username=$1 OR opp_username=$1) AND STATUS='ACCEPTED' ORDER BY created_on DESC LIMIT 100")
        .bind(&user.username)
        .fetch_all(&**pool).await;
    let challenges = match challenges_result {
        Ok(cs) => cs,
        Err(e) => {
            println!("error getting challenges: {}", e);
            return
        }
    };

    // 2. check in lichess if there are any updates
    // 2. TODO: parallelize this
    for challenge in challenges.iter() {
        println!("processing challenge {}", challenge.id);

        let url = format!("https://lichess.org/game/export/{}", &challenge.lichess_challenge_id.as_ref().unwrap());
        let access_token = &user.access_token;
        let bearer = format!("Bearer {access_token}");
        let resp = Client::new()
            .get(url)
            .header("Authorization", bearer)
            .header("Accept", "application/json")
            .send().await;

        let mut lichess_export_game_response: LichessExportGameResponse = match resp {
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
                        println!("error getting game on lichess text(): {}", e);
                        return;
                    }
                }
            },
            Err(e) => {
                println!("error getting game on lichess : {}", e);
                return;
            }
        };

        let challenge_lichess_result = lichess_export_game_response.status;
        if challenge_lichess_result == "created" || challenge_lichess_result == "started" {
            println!("challenge not over yet {}", &challenge.lichess_challenge_id.as_ref().unwrap());
            continue;
        }

        // determine fee
        let initial_fee: f64 = (challenge.sats.unwrap() as f64) * 0.02;
        let rounded_down = initial_fee.floor() as i64;
        // make even
        let fee = rounded_down - rounded_down % 2;
        let admin_result = env::var("ADMIN_ACCOUNT");
        let admin = match admin_result {
            Ok(a) => a,
            Err(e) => {
                println!("error getting admin account: {}", e);
                return;
            }
        };

        let tx_result = pool.begin().await;
        let mut tx = match tx_result {
            Ok(t) => t,
            Err(e) => {
                println!("error creating tx: {}", e);
                return;
            }
        };

        // pay admin
        let admin_ttype = "fee";
        let admin_detail = format!("fee from challenge {}", challenge.id);
        let admin_state = "SETTLED";
        let admin_transaction_result = sqlx::query( "INSERT INTO lightningchess_transaction (username, ttype, detail, amount, state, lichess_challenge_id) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(&admin)
            .bind(admin_ttype)
            .bind(admin_detail)
            .bind(fee)
            .bind(admin_state)
            .bind(&challenge.lichess_challenge_id.as_ref().unwrap())
            .execute(&mut tx).await;

        match admin_transaction_result {
            Ok(_) => println!("insert transaction successfully"),
            Err(e) => {
                println!("insert transaction failed {}", e);
                return;
            }
        }

        let admin_balance = sqlx::query( "UPDATE lightningchess_balance set balance=balance + $1 WHERE username=$2")
            .bind(fee)
            .bind(&admin)
            .execute(&mut tx).await;

        match admin_balance {
            Ok(_) => println!("successfully payed admin"),
            Err(e) => {
                println!("error paying admin admin_transaction transaction{}", e);
                return;
            }
        }

        let winner = lichess_export_game_response.winner.get_or_insert("".to_string());
        if winner == "black" || winner == "white" {
            // pay money to winner
            let winner_username = if challenge.color.as_ref().unwrap() == "black" { &challenge.username } else { &challenge.opp_username };
            let winner_ttype = "winnings";
            let winner_detail = "";
            let winning_amt = (&challenge.sats.unwrap() * 2) - fee;
            let winner_state = "SETTLED";
            let winner_transaction_result = sqlx::query( "INSERT INTO lightningchess_transaction (username, ttype, detail, amount, state) VALUES ($1, $2, $3, $4, $5)")
                .bind(winner_username)
                .bind(winner_ttype)
                .bind(winner_detail)
                .bind(winning_amt)
                .bind(winner_state)
                .execute(&mut tx).await;

            match winner_transaction_result {
                Ok(_) => println!("insert transaction successfully"),
                Err(e) => {
                    println!("insert transaction failed {}", e);
                    return;
                }
            }

            let winner_balance = sqlx::query( "UPDATE lightningchess_balance set balance=balance + $1 WHERE username=$2")
                .bind(winning_amt)
                .bind(winner_username)
                .execute(&mut tx).await;

            match winner_balance {
                Ok(_) => println!("successfully payed admin"),
                Err(e) => {
                    println!("error paying admin admin_transaction transaction{}", e);
                    return;
                }
            }
        } else {
            // no winner so return money to both people
            let draw_ttype = "draw";
            let draw_detail = "initial sats amount minus 2% fee";
            let draw_amt = &challenge.sats.unwrap() - (fee / 2);
            let draw_state = "SETTLED";
            let draw_transaction_result = sqlx::query( "INSERT INTO lightningchess_transaction (username, ttype, detail, amount, state) VALUES ($1, $2, $3, $4, $5)")
                .bind(&challenge.username)
                .bind(draw_ttype)
                .bind(draw_detail)
                .bind(draw_amt)
                .bind(draw_state)
                .execute(&mut tx).await;

            match draw_transaction_result {
                Ok(_) => println!("insert transaction successfully"),
                Err(e) => {
                    println!("insert transaction failed {}", e);
                    return;
                }
            }

            let draw_balance = sqlx::query( "UPDATE lightningchess_balance set balance=balance + $1 WHERE username=$2")
                .bind(draw_amt)
                .bind(&challenge.username)
                .execute(&mut tx).await;

            match draw_balance {
                Ok(_) => println!("successfully payed draw 1"),
                Err(e) => {
                    println!("error paying draw 1 balance transaction{}", e);
                    return;
                }
            }

            let draw_transaction_result2 = sqlx::query( "INSERT INTO lightningchess_transaction (username, ttype, detail, amount, state) VALUES ($1, $2, $3, $4, $5)")
                .bind(&challenge.opp_username)
                .bind(draw_ttype)
                .bind(draw_detail)
                .bind(draw_amt)
                .bind(draw_state)
                .execute(&mut tx).await;

            match draw_transaction_result2 {
                Ok(_) => println!("insert transaction successfully"),
                Err(e) => {
                    println!("insert transaction failed {}", e);
                    return;
                }
            }

            let draw_balance2 = sqlx::query( "UPDATE lightningchess_balance set balance=balance + $1 WHERE username=$2")
                .bind(draw_amt)
                .bind(&challenge.opp_username)
                .execute(&mut tx).await;

            match draw_balance2 {
                Ok(_) => println!("successfully payed draw 2"),
                Err(e) => {
                    println!("error paying draw 2 balance transaction{}", e);
                    return;
                }
            }
        }

        // mark challenge as completed
        // update challenge in db
        let status = "COMPLETED";
        let pg_query_result = sqlx::query_as::<_,Challenge>("UPDATE challenge SET status=$1 WHERE id=$2 RETURNING *")
            .bind(status)
            .bind(&challenge.id)
            .fetch_one(&mut tx).await;

        match pg_query_result {
            Ok(_) => println!("update challenge succeeded"),
            Err(e) => {
                println!("update challenge failed: {}", e);
                return;
            }
        };

        // commit transaction, return challenge
        let commit_result = tx.commit().await;
        match commit_result {
            Ok(_) => println!("successfully committed"),
            Err(e) => println!("error committing: {}", e)
        }
    }
}

#[post("/api/send-payment", data = "<send_payment_request_str>")]
pub async fn send_payment_endpoint(user: User, pool: &State<Pool<Postgres>>, send_payment_request_str: String) -> Result<String, Status> {
    println!("send_payment_request_str: {}", send_payment_request_str);
    let send_payment_result: Result<SendPaymentRequest, serde_json::Error> = serde_json::from_str(&send_payment_request_str);
    let send_payment = match send_payment_result {
        Ok(sp) => sp,
        Err(e) => {
            println!("error: {}", e);
            return Err(Status::BadRequest)
        }
    };

    // decode
    let decoded_option = decode_payment(&send_payment.payment_request).await;
    let decoded_payment = match decoded_option {
        Some(dp) => dp,
        None => return Err(Status::BadRequest)
    };
    let withdrawal_amt = decoded_payment.num_satoshis.parse::<i64>().unwrap();

    // not sure if this is possible
    if withdrawal_amt < 0 {
        return Err(Status::BadRequest);
    }

    let balance_result = sqlx::query_as::<_,Balance>( "SELECT * FROM lightningchess_balance WHERE username=$1")
        .bind(&user.username)
        .fetch_one(&**pool).await;
    let balance = match balance_result {
        Ok(b) => b,
        Err(e) => {
            println!("error: {}", e);
            return Err(Status::InternalServerError);
        }
    };

    // only send if they have enough money
    if balance.balance <= withdrawal_amt {
        return Err(Status::BadRequest);
    }
    // if there are any existing open payments, pay them and return fail for this

    let withdrawal_amt_neg = withdrawal_amt * -1;

    // insert payment into transactions table with status == 0PEN, commit
    // if we don't do this, we never have a way to retry if the update the db fails after the payment is made
    let withdrawal_ttype = "withdrawal";
    let withdrawal_detail = "";
    let withdrawal_state = "OPEN";
    let withdrawal_transaction_result = sqlx::query_as::<_, Transaction>( "INSERT INTO lightningchess_transaction (username, ttype, detail, amount, state, payment_hash) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *")
        .bind(&user.username)
        .bind(withdrawal_ttype)
        .bind(withdrawal_detail)
        .bind(&withdrawal_amt_neg)
        .bind(withdrawal_state)
        .bind(&decoded_payment.payment_hash)
        .fetch_one(&**pool).await;

    let withdrawal_transaction = match withdrawal_transaction_result {
        Ok(t) => {
            println!("withdrawal transaction insert successfully");
            t
        },
        Err(e) => {
            println!("insert transaction failed {}", e);
            return Err(Status::BadRequest);
        }
    };

    // send payment to lightning node
    make_payment(&send_payment.payment_request).await;

    let tx_result = pool.begin().await;
    let mut tx = match tx_result {
        Ok(t) => t,
        Err(e) => {
            println!("error creating tx: {}", e);
            return Err(Status::BadRequest);
        }
    };

    let new_state = "SETTLED";
    let updated_transaction = sqlx::query( "UPDATE lightningchess_transaction SET state=$1, amount=$2 WHERE transaction_id=$3")
        .bind(new_state)
        .bind(&withdrawal_amt_neg)
        .bind(withdrawal_transaction.transaction_id)
        .execute(&mut tx).await;

    match updated_transaction {
        Ok(_) => println!("successfully updated_transaction transaction id"),
        Err(e) => {
            println!("error updated_transaction transaction id : {}", e);
            return Err(Status::InternalServerError);
        }
    }

    let winner_balance = sqlx::query( "UPDATE lightningchess_balance set balance=balance + $1 WHERE username=$2")
        .bind(&withdrawal_amt_neg)
        .bind(&user.username)
        .execute(&mut tx).await;

    match winner_balance {
        Ok(_) => println!("successfully payed admin"),
        Err(e) => {
            println!("error paying admin admin_transaction transaction{}", e);
            return Err(Status::InternalServerError);
        }
    }

    // commit transaction
    let commit_result = tx.commit().await;
    return match commit_result {
        Ok(_) => {
            println!("successfully committed");
            let send_payment_response = SendPaymentResponse {
                complete: true
            };
            Ok(serde_json::to_string(&send_payment_response).unwrap())
        },
        Err(e) => {
            println!("error committing: {}", e);
            Err(Status::InternalServerError)
        }
    }
}
