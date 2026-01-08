use std::fs;

use axum_test::{
    multipart::{MultipartForm, Part},
    TestResponse, TestServer,
};
use darve_server::entities::task_request_user::TaskParticipant;

#[allow(dead_code)]
pub async fn success_deliver_task(
    server: &TestServer,
    task_id: &str,
    user_token: &str,
) -> Result<TaskParticipant, String> {
    let res = deliver_task(server, task_id, user_token).await;
    res.assert_status_success();
    Ok(res.json::<TaskParticipant>())
}

#[allow(dead_code)]
pub async fn deliver_task(server: &TestServer, task_id: &str, user_token: &str) -> TestResponse {
    let file = fs::read("tests/dummy/file_example_PNG_1MB.png").unwrap();
    let part = Part::bytes(file)
        .file_name("file_example_PNG_1MB.png")
        .mime_type("image/jpeg");
    let data = MultipartForm::new().add_part("content", part);
    server
        .post(&format!("/api/tasks/{}/deliver", task_id))
        .multipart(data)
        .add_header("Cookie", format!("jwt={}", user_token))
        .add_header("Accept", "application/json")
        .await
}
