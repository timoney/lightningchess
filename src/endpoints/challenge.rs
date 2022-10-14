use rocket::http::{Cookie, CookieJar};
use crate::models::{Challenge};

#[post("/api/challenge", data = "<challenge_request>")]
pub async fn challenge(challenge_request: String, cookies: &CookieJar<'_>) -> String {
    println!("challenge request!: {}", challenge_request);
    let challenge: Challenge = serde_json::from_str(&challenge_request).unwrap();
    println!("challenge!{}", serde_json::to_string(&challenge).unwrap());
    "hi".to_string()
    // save to db
    // let url = format!("https://lichess.org/api/challenge/{}", &challenge.opponent);
    // let access_token = cookies.get("access_token").map(|c| c.value()).unwrap();
    // let bearer = format!("Bearer {access_token}");
    // let body = LichessChallenge {
    //     rated: true,
    //     clock: LichessChallengeClock {
    //         limit: challenge.limit.to_string(),
    //         increment: challenge.increment.to_string(),
    //     },
    //     color: challenge.color,
    //     variant: "standard".to_string(),
    //     rules: "noClaimWin".to_string(),
    // };
    // println!("url: {}", url);
    // println!("body: {}", serde_json::to_string(&body).unwrap());
    // println!("bearer: {}", bearer);
    // let resp = Client::new()
    //     .post(url)
    //     .json(&body)
    //     .header("Authorization", bearer)
    //     .send().await;
    //
    // return match resp {
    //     Ok(res) => {
    //         println!("Status: {}", res.status());
    //         println!("Headers:\n{:#?}", res.headers());
    //
    //         let text = res.text().await;
    //         match text {
    //             Ok(text) => {
    //                 println!("text!: {}", text);
    //                 let resp: LichessChallengeResponse = serde_json::from_str(&text).unwrap();
    //                 serde_json::to_string(&resp).unwrap()
    //             }
    //             Err(e) => {
    //                 println!("error:\n{}", e);
    //                 "error:(".to_string()
    //             }
    //         }
    //     },
    //     Err(error) => {
    //         println!("error:\n{}", error);
    //         "error:(:(".to_string()
    //     }
    // }
}