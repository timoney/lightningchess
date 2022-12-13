use crate::models::{User, AddHoldInvoiceResponse};
use rand::{distributions::Alphanumeric, Rng};
use reqwest::Client;
use serde_json::json;
use std::str;
use crate::invoicesrpc::AddHoldInvoiceRequest;

pub async fn add_invoice(sats: i64) -> Option<AddHoldInvoiceResponse> {
    let macaroon = env!("LND_MACAROON");
    let rand: Vec<u8>  = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(43)
        .collect();

    let rand_str = str::from_utf8(&rand).unwrap();
    let sats_str = sats.to_string();
    let body = json!({
        "hash": rand_str,
        "value": sats_str,
    });
    println!("body: {}", body.to_string());

    let response = Client::new()
        .post("https://lightningchess.m.voltageapp.io:8080/v2/invoices/hodl")
        .json(&body)
        .header("Grpc-Metadata-macaroon", macaroon)
        .send().await;

    match response {
        Ok(res) => {
            println!("Status: {}", res.status());
            println!("Headers:\n{:#?}", res.headers());
            let text = res.text().await;
            match text {
                Ok(text) => {
                    println!("text: {}", text);
                    let json_response: AddHoldInvoiceResponse = serde_json::from_str(&text).unwrap();
                    println!("addHoldInvoiceResponse: {}", serde_json::to_string(&json_response).unwrap());
                    Some(json_response)
                }
                Err(e) => {
                    println!("error in text() :\n{}", e);
                    None
                }
            }
        },
        Err(e) => {
            println!("error from lnd addHoldInvoice\n{}", e);
            None
        }
    }
}