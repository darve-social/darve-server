use axum_test::TestServer;
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
