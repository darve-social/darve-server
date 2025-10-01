pub mod post_helpers;
pub mod test_with_server;
pub mod user_helpers;

use axum_test::{multipart::MultipartForm, TestServer};
use chrono::{DateTime, Months, Utc};
use darve_server::{
    entities::user_auth::local_user_entity::LocalUser, utils::blocked_words::BLOCKED_WORDS,
};
use fake::{faker, Fake};
use serde_json::Value;

#[allow(dead_code)]
pub async fn create_login_test_user(
    server: &TestServer,
    username: String,
) -> (&TestServer, String) {
    let data = MultipartForm::new()
        .add_text("username", username.as_str())
        .add_text("password", "some3242paSs#$");

    let create_user = &server.post("/api/register").multipart(data).await;

    println!("Creating user with username: {username} {:?}", create_user);
    create_user.assert_status_success();
    let auth_response = create_user.json::<Value>();
    let user = serde_json::from_value::<LocalUser>(auth_response["user"].clone()).unwrap();
    (server, user.id.unwrap().to_raw())
}

#[allow(dead_code)]
pub async fn create_fake_login_test_user(
    server: &TestServer,
) -> (&TestServer, LocalUser, String, String) {
    let pwd = faker::internet::en::Password(6..8).fake::<String>();
    let birthday = faker::chrono::en::DateTimeBetween(
        Utc::now()
            .checked_sub_months(Months::new(12 * 100))
            .unwrap(),
        Utc::now().checked_sub_months(Months::new(12 * 10)).unwrap(),
    )
    .fake::<DateTime<Utc>>();
    let data = MultipartForm::new()
        .add_text("username", fake_username_min_len(6).as_str())
        .add_text("password", pwd.as_str())
        .add_text(
            "full_name",
            faker::name::en::Name().fake::<String>().as_str(),
        )
        .add_text("birth_day", birthday.to_string());

    let create_user = &server.post("/api/register").multipart(data).await;

    create_user.assert_status_success();
    let auth_response = create_user.json::<Value>();
    let user = serde_json::from_value::<LocalUser>(auth_response["user"].clone()).unwrap();
    (
        server,
        user,
        pwd,
        auth_response["token"]
            .to_string()
            .trim_matches('"')
            .to_string(),
    )
}

#[allow(dead_code)]
pub fn fake_username_min_len(min_len: usize) -> String {
    use fake::{faker::internet::en::Username, Fake};
    (0..)
        .map(|_| Username().fake::<String>().replace(".", "_"))
        .find(|u| u.len() >= min_len && !BLOCKED_WORDS.contains(u))
        .unwrap()
}
