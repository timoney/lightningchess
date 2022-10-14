use crate::models::{User, UserProfile};

#[get("/profile")]
pub async fn profile(user: User) -> String {
    let use_profile: UserProfile = UserProfile {
        username: user.username
    };
    serde_json::to_string(&use_profile).unwrap()
}