use rand::distributions::Alphanumeric;
use rand::Rng;
use rocket::http::Status;
use rocket::State;
use sqlx::{Pool, Postgres};
use crate::models::{Transaction, AddInvoiceRequest, User, Balance};
use crate::lightning::invoices::add_invoice;

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

#[get("/api/transactions")]
pub async fn transactions(user: User, pool: &State<Pool<Postgres>>) -> Result<String, Status> {
    let challenges = sqlx::query_as::<_,Transaction>( "SELECT * FROM lightningchess_transaction WHERE username=$1 ORDER BY transaction_id DESC LIMIT 100")
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

#[get("/api/balance")]
pub async fn balance(user: User, pool: &State<Pool<Postgres>>) -> Result<String, Status> {
    let balance_result = sqlx::query_as::<_,Balance>( "SELECT balance FROM lightningchess_balance WHERE username=$1")
        .bind(&user.username)
        .fetch_one(&**pool).await;
    match balance_result {
        Ok(balance) => Ok(serde_json::to_string(&balance).unwrap()),
        Err(e) => {
            println!("error: {}", e);
            Err(Status::InternalServerError)
        }
    }
}

