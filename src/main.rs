#[macro_use] extern crate rocket;

use crate::config::parse_config;
use crate::endpoints::callback::callback;
use crate::endpoints::challenge::{accept_challenge, create_challenge, lookup_challenge, challenges};
use crate::endpoints::money::{add_invoice_endpoint, balance, transactions};
use crate::endpoints::login::login;
use crate::endpoints::profile::profile;
use crate::models::AppConfig;
use rocket::fairing::AdHoc;
use rocket::State;
use rocket_dyn_templates::Template;
use std::collections::HashMap;
use sqlx::postgres::PgPoolOptions;

pub mod guard;
pub mod models;
pub mod lightning;
pub mod endpoints;
pub mod config;

#[get("/")]
async fn index(app_config: &State<AppConfig>) -> Template {
    let mut context = HashMap::new();
    context.insert("fe_url", app_config.fe_url.to_string());
    Template::render("index", &context)
}

#[launch]
async fn rocket() -> _ {
    let database_url = "postgresql://postgres:example@localhost:5432/postgres";

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await.unwrap();

    rocket::build()
        .attach(AdHoc::try_on_ignite("appConfig", parse_config))
        .manage(pool)
        .mount("/", routes![
            index,
            login,
            callback,
            profile,
            create_challenge,
            accept_challenge,
            lookup_challenge,
            challenges,
            add_invoice_endpoint,
            balance,
            transactions])
        .attach(Template::fairing())
}
