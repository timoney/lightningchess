#[macro_use] extern crate rocket;

use crate::config::parseConfig;
use crate::endpoints::callback::callback;
use crate::endpoints::challenge::challenge;
use crate::endpoints::login::login;
use crate::endpoints::profile::profile;
use crate::models::AppConfig;
use rocket::fairing::AdHoc;
use rocket::State;
use rocket_dyn_templates::Template;
use std::collections::HashMap;

pub mod guard;
pub mod models;
pub mod endpoints;
pub mod config;

#[get("/")]
fn index(app_config: &State<AppConfig>,) -> Template {
    let mut context = HashMap::new();
    context.insert("fe_url", app_config.fe_url.to_string());
    Template::render("index", &context)
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(AdHoc::try_on_ignite("appConfig", parseConfig))
        .mount("/", routes![index, login, callback, profile, challenge])
        .attach(Template::fairing())
}
