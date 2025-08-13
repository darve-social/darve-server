use axum_test::TestServer;
use darve_server::{
    entities::community::discussion_entity::DiscussionDbService,
    middleware::utils::string_utils::get_string_thing,
};
use surrealdb::sql::Thing;

#[allow(dead_code)]
pub struct CreateFakeCommunityResponse {
    pub id: String,
    pub name: String,
    pub default_discussion: Thing,
}

#[allow(dead_code)]
pub async fn get_profile_discussion_id(server: &TestServer, user_ident: String) -> Thing {
    let create_response = server
        .get(&format!("/u/{}", user_ident))
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_success();
    return DiscussionDbService::get_profile_discussion_id(&get_string_thing(user_ident).unwrap());
}
