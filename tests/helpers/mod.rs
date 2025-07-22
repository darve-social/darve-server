pub mod community_helpers;
pub mod post_helpers;
pub mod test_with_server;
pub mod user_helpers;

use axum_test::TestServer;
use chrono::{DateTime, Utc};
use darve_server::entities::user_auth::local_user_entity::LocalUser;
use fake::{faker, Fake};
use serde_json::{json, Value};

#[allow(dead_code)]
pub async fn create_login_test_user(
    server: &TestServer,
    username: String,
) -> (&TestServer, String) {
    let create_user = &server
        .post("/api/register")
        .json(
            &json!({ "username": username.to_string(),  "password": "some3242paSs#$".to_string()}),
        )
        .await;

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
    let username = fake_username_min_len(6);
    let create_user = &server
        .post("/api/register")
        .json(&json!({
            "username": username,
            "password": pwd.clone(),
            "email": faker::internet::en::FreeEmail().fake::<String>(),
            "full_name": faker::name::en::Name().fake::<String>(),
            "birth_day": faker::chrono::en::DateTime().fake::<DateTime<Utc>>()
        }))
        .await;

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
        .find(|u| u.len() >= min_len)
        .unwrap()
}
