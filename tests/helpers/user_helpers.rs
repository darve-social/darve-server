use std::fs;

use axum_test::{
    multipart::{MultipartForm, Part},
    TestResponse, TestServer,
};
use darve_server::{models::view::user::UserView, routes::users::SearchInput};

#[allow(dead_code)]
pub async fn search_users(server: &TestServer, input: &SearchInput) -> Vec<UserView> {
    let request = server
        .get("/api/users")
        .add_query_param("query", input.query.clone())
        .add_header("Accept", "application/json")
        .await;

    request.assert_status_success();
    request.json::<Vec<UserView>>()
}

#[allow(dead_code)]
pub async fn update_current_user(server: &TestServer) -> TestResponse {
    let file = fs::read("tests/dummy/file_example_PNG_1MB.png").unwrap();
    let part = Part::bytes(file)
        .file_name("file_example_PNG_1MB.png")
        .mime_type("image/png");
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
