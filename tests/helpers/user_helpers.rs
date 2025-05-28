use std::fs;

use axum_test::{
    multipart::{MultipartForm, Part},
    TestResponse, TestServer,
};
use darve_server::routes::{
    community::profile_routes::SearchInput, user_auth::follow_routes::UserListView,
};

#[allow(dead_code)]
pub async fn create_user(server: &TestServer, input: &SearchInput) -> UserListView {
    let request = server
        .post("/api/user/search")
        .json(input)
        .add_header("Accept", "application/json")
        .await;
    request.assert_status_success();
    request.json::<UserListView>()
}

#[allow(dead_code)]
pub async fn update_current_user(server: &TestServer) -> TestResponse {
    let file = fs::read("tests/dummy/test_image_2mb.jpg").unwrap();
    let part = Part::bytes(file)
        .file_name("test_image_2mb.jpg")
        .mime_type("image/jpeg");
    let data = MultipartForm::new().add_part("image_url", part);

    server
        .post("/api/accounts/edit")
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
