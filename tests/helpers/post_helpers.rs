use axum_test::{multipart::Part, TestServer};
use darve_server::middleware::{self, mw_ctx::CtxState, utils::request_utils::CreatedResponse};
use fake::{faker, Fake};
use surrealdb::sql::Thing;

pub async fn create_fake_post(server: &TestServer, discussion_id: &Thing) -> String {
    use axum_test::multipart::MultipartForm;
    use middleware::utils::request_utils::CreatedResponse;

    let post_name = faker::name::en::Name().fake::<String>();
    let content = faker::lorem::en::Sentence(7..20).fake::<String>();

    let data = MultipartForm::new()
        .add_text("title", post_name.clone())
        .add_text("content", content)
        .add_text("topic_id", "");

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
