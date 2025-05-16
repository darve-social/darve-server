use axum_test::{multipart::Part, TestServer};
use darve_server::middleware::{self, mw_ctx::CtxState, utils::request_utils::CreatedResponse};
use fake::{faker, Fake};
use std::fs;
use surrealdb::sql::Thing;

#[allow(dead_code)]
pub struct CreateFakePostResponse {
    pub id: String,
    pub uri: String,
}

#[allow(dead_code)]
pub async fn create_fake_post(server: &TestServer, discussion_id: &Thing, topic_id: Option<Thing>) -> CreateFakePostResponse {
    use axum_test::multipart::MultipartForm;
    use middleware::utils::request_utils::CreatedResponse;

    let post_name = faker::name::en::Name().fake::<String>();
    let content = faker::lorem::en::Sentence(7..20).fake::<String>();

    let data = MultipartForm::new()
        .add_text("title", post_name.clone())
        .add_text("content", content)
        .add_text("topic_id", topic_id.map(|v|v.to_raw()).unwrap_or_default());

    let create_post = server
        .post(format!("/api/discussion/{discussion_id}/post").as_str())
        .multipart(data)
        .add_header("Accept", "application/json")
        .await;

    let created = create_post.json::<CreatedResponse>();
    let _ = create_post.assert_status_success();
    assert_eq!(created.id.len() > 0, true);

    CreateFakePostResponse{id: created.id, uri: created.uri.unwrap()}
}

#[allow(dead_code)]
pub async fn create_fake_post_with_large_file(
    server: &TestServer,
    ctx_state: &CtxState,
    discussion_id: &Thing,
) {
    use axum_test::multipart::MultipartForm;

    let _ = darve_server::utils::dir_utils::ensure_dir_exists(&ctx_state.uploads_dir);

    let post_name = faker::name::en::Name().fake::<String>();
    let content = faker::lorem::en::Sentence(7..20).fake::<String>();
    let file = fs::read("tests/dummy/test_image_20mb.jpg").unwrap();

    let part = Part::bytes(file)
        .file_name("test_image_20mb.jpg")
        .mime_type("image/jpeg");

    let data = MultipartForm::new()
        .add_text("title", post_name.clone())
        .add_text("content", content)
        .add_text("topic_id", "")
        .add_part("file_1", part);

    let response = server
        .post(format!("/api/discussion/{discussion_id}/post").as_str())
        .multipart(data)
        .add_header("Accept", "application/json")
        .await;

    response.assert_status_payload_too_large();
}

#[allow(dead_code)]
pub async fn create_fake_post_with_file(
    server: &TestServer,
    ctx_state: &CtxState,
    discussion_id: &Thing,
) -> String {
    use axum_test::multipart::MultipartForm;
    let _ = darve_server::utils::dir_utils::ensure_dir_exists(&ctx_state.uploads_dir);

    let post_name = faker::name::en::Name().fake::<String>();
    let content = faker::lorem::en::Sentence(7..20).fake::<String>();

    let file = fs::read("tests/dummy/test_image_2mb.jpg").unwrap();

    let part = Part::bytes(file)
        .file_name("test_image_2mb.jpg")
        .mime_type("image/jpeg");

    let data = MultipartForm::new()
        .add_text("title", post_name.clone())
        .add_text("content", content)
        .add_text("topic_id", "")
        .add_part("file_1", part);

    let create_post = server
        .post(format!("/api/discussion/{discussion_id}/post").as_str())
        .multipart(data)
        .add_header("Accept", "application/json")
        .await;

    let created = create_post.json::<CreatedResponse>();
    let _ = create_post.assert_status_success();
    assert_eq!(created.id.len() > 0, true);

    created.id
}
