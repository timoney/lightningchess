pub mod auth {
    use reqwest::Client;
    use rocket::Request;
    use rocket::request::{FromRequest, Outcome};
    use crate::{Account, User};

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

}