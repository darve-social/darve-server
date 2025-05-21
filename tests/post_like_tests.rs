mod helpers;

use crate::helpers::{create_login_test_user, create_test_server};
use darve_server::routes::community::post_routes::PostLikeResponse;
use helpers::community_helpers::create_fake_community;
use helpers::post_helpers::{self, create_fake_post};

#[tokio::test]
async fn create_post_like() {
    let (server, ctx_state) = create_test_server().await;
    let (server, user_ident) = create_login_test_user(&server, "usnnnn".to_string()).await;
    let result = create_fake_community(server, &ctx_state, user_ident.clone()).await;

    let result = create_fake_post(server, &result.profile_discussion, None, None).await;

    let response = post_helpers::create_post_like(&server, &result.id).await;
    response.assert_status_ok();
    let likes_nr = response.json::<PostLikeResponse>().likes_count;
    assert_eq!(likes_nr, 1);

    let response = post_helpers::create_post_like(&server, &result.id).await;
    response.assert_status_failure();

    let response = post_helpers::delete_post_like(&server, &result.id).await;
    response.assert_status_ok();

    let likes_nr = response.json::<PostLikeResponse>().likes_count;

    assert_eq!(likes_nr, 0);
}
