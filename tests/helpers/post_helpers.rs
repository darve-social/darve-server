use axum_test::multipart::MultipartForm;
use axum_test::TestResponse;
use axum_test::{multipart::Part, TestServer};
use darve_server::middleware::{mw_ctx::CtxState, utils::request_utils::CreatedResponse};
use darve_server::routes::community::post_routes::GetPostsQuery;
use fake::{faker, Fake};
use std::fs;
use surrealdb::sql::Thing;

#[allow(dead_code)]
pub async fn create_post(
    server: &TestServer,
    discussion_id: &Thing,
    data: MultipartForm,
) -> TestResponse {
    server
        .post(format!("/api/discussion/{discussion_id}/post").as_str())
        .multipart(data)
        .add_header("Accept", "application/json")
        .await
}

#[allow(dead_code)]
pub async fn get_posts(server: &TestServer, query: Option<GetPostsQuery>) -> TestResponse {
    let params = match query {
        Some(query) => {
            let mut params = Vec::new();

            if let Some(tag) = query.tag {
                params.push(format!("tag={}", tag));
            }

            if let Some(start) = query.start {
                params.push(format!("start={}", start));
            }

            if let Some(count) = query.count {
                params.push(format!("count={}", count));
            }

            if params.is_empty() {
                String::new()
            } else {
                format!("?{}", params.join("&"))
            }
        }
        None => String::new(),
    };

    server
        .get(format!("/api/posts{params}").as_str())
        .add_header("Accept", "application/json")
        .await
}

#[allow(dead_code)]
pub struct CreateFakePostResponse {
    pub id: String,
    pub uri: String,
}

#[allow(dead_code)]
pub fn build_fake_post(topic_id: Option<Thing>, tags: Option<Vec<String>>) -> MultipartForm {
    let post_name = faker::name::en::Name().fake::<String>();
    let content = faker::lorem::en::Sentence(7..20).fake::<String>();
    let mut data = MultipartForm::new();
    data = data.add_text("title", post_name.clone());
    data = data.add_text("content", content);
    data = data.add_text("topic_id", topic_id.map(|v| v.to_raw()).unwrap_or_default());
    let tags = tags.unwrap_or(vec![]);
    for tag in tags.into_iter() {
        data = data.add_text("tags", tag);
    }
    data
}

#[allow(dead_code)]
pub async fn create_fake_post(
    server: &TestServer,
    discussion_id: &Thing,
    topic_id: Option<Thing>,
    tags: Option<Vec<String>>,
) -> CreateFakePostResponse {
    let data = build_fake_post(topic_id, tags);
    let create_post = create_post(&server, &discussion_id, data).await;
    let created = create_post.json::<CreatedResponse>();
    let _ = create_post.assert_status_success();
    assert_eq!(created.id.len() > 0, true);

    CreateFakePostResponse {
        id: created.id,
        uri: created.uri.unwrap(),
    }
}

#[allow(dead_code)]
pub async fn create_fake_post_with_large_file(
    server: &TestServer,
    ctx_state: &CtxState,
    discussion_id: &Thing,
) {
    let _ = darve_server::utils::dir_utils::ensure_dir_exists(&ctx_state.uploads_dir);
    let mut data = build_fake_post(None, None);
    let file = fs::read("tests/dummy/test_image_20mb.jpg").unwrap();
    let part = Part::bytes(file)
        .file_name("test_image_20mb.jpg")
        .mime_type("image/jpeg");
    data = data.add_part("file_1", part);
    let response = create_post(&server, &discussion_id, data).await;

    response.assert_status_payload_too_large();
}

#[allow(dead_code)]
pub async fn create_fake_post_with_file(
    server: &TestServer,
    ctx_state: &CtxState,
    discussion_id: &Thing,
) -> String {
    let _ = darve_server::utils::dir_utils::ensure_dir_exists(&ctx_state.uploads_dir);

    let mut data = build_fake_post(None, None);
    let file = fs::read("tests/dummy/test_image_2mb.jpg").unwrap();

    let part = Part::bytes(file)
        .file_name("test_image_2mb.jpg")
        .mime_type("image/jpeg");

    data = data.add_part("file_1", part);
    let response = create_post(&server, &discussion_id, data).await;
    let _ = response.assert_status_success();
    let created = response.json::<CreatedResponse>();

    assert_eq!(created.id.len() > 0, true);

    created.id
}

#[allow(dead_code)]
pub async fn create_post_like(server: &TestServer, post_id: &str) -> TestResponse {
    server
        .post(format!("/api/posts/{post_id}/like").as_str())
        .add_header("Accept", "application/json")
        .await
}

#[allow(dead_code)]
pub async fn delete_post_like(server: &TestServer, post_id: &str) -> TestResponse {
    server
        .delete(format!("/api/posts/{post_id}/unlike").as_str())
        .add_header("Accept", "application/json")
        .await
}
