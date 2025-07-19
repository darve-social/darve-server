use std::fs;

use axum_test::{
    multipart::{MultipartForm, Part},
    TestResponse, TestServer,
};
use darve_server::{entities::user_auth::local_user_entity::LocalUser, routes::users::SearchInput};

#[allow(dead_code)]
pub async fn search_users(server: &TestServer, input: &SearchInput) -> Vec<LocalUser> {
    let request = server
        .get("/api/users")
        .json(input)
        .add_header("Accept", "application/json")
        .await;
    request.assert_status_success();
    request.json::<Vec<LocalUser>>()
}

#[allow(dead_code)]
pub async fn update_current_user(server: &TestServer) -> TestResponse {
    let file = fs::read("tests/dummy/test_image_2mb.jpg").unwrap();
    let part = Part::bytes(file)
        .file_name("test_image_2mb.jpg")
        .mime_type("image/jpeg");
    let data = MultipartForm::new().add_part("image_url", part);

    server
        .patch("/api/users/current")
        .add_header("Accept", "application/json")
        .multipart(data)
        .await
}

#[allow(dead_code)]
pub async fn get_user(server: &TestServer, user_id: &str) -> TestResponse {
    server
        .get(&format!("/u/{}", user_id))
        .add_header("Accept", "application/json")
        .await
}
