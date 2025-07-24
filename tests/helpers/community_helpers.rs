use axum_test::TestServer;
use darve_server::{
    entities::community::{
        community_entity::{self, CommunityDbService},
        discussion_entity::DiscussionDbService,
    },
    middleware::{self, ctx::Ctx, mw_ctx::CtxState, utils::string_utils::get_string_thing},
    routes::community::community_routes,
    services::discussion_service::CreateDiscussion,
};
use fake::{faker, Fake};
use surrealdb::sql::Thing;

#[allow(dead_code)]
pub struct CreateFakeCommunityResponse {
    pub id: String,
    pub name: String,
    pub default_discussion: Thing,
}

#[allow(dead_code)]
pub async fn create_fake_community(
    server: &TestServer,
    ctx_state: &CtxState,
    user_ident: String,
) -> CreateFakeCommunityResponse {
    use community_entity::{Community, CommunityDbService};
    use community_routes::CommunityInput;
    use middleware::utils::request_utils::CreatedResponse;

    let comm_name = faker::name::en::Name().fake::<String>().to_lowercase();
    let title = faker::lorem::en::Sentence(5..10).fake::<String>();

    let create_response = server
        .post("/api/community")
        .json(&CommunityInput {
            id: "".to_string(),
            name_uri: comm_name.clone(),
            title,
        })
        .add_header("Accept", "application/json")
        .await;

    let created = &create_response.json::<CreatedResponse>();

    let comm_id = Thing::try_from(created.id.clone()).unwrap();
    let comm_name = created.uri.clone().unwrap();
    let _ = create_response.assert_status_success();

    let ctx = Ctx::new(Ok(user_ident), false);

    let community_db_service = CommunityDbService {
        db: &ctx_state.db.client,
        ctx: &ctx,
    };

    let community: Community = community_db_service
        .db
        .select((&comm_id.tb, comm_id.id.to_raw()))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(comm_name, community.name_uri.clone());

    CreateFakeCommunityResponse {
        id: created.id.clone(),
        name: comm_name,
        default_discussion: community.default_discussion.clone().unwrap(),
    }
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

#[allow(dead_code)]
pub struct CreatePrivateDiscussionResponse {
    pub id: Thing,
    pub title: String,
    pub participants: Vec<Thing>,
}

#[allow(dead_code)]
pub async fn create_private_discussion(
    server: &TestServer,
    creator_id: Thing,
    participant_ids: Vec<String>,
    creator_token: String,
) -> CreatePrivateDiscussionResponse {
    use darve_server::entities::community::discussion_entity::Discussion;

    let title = faker::lorem::en::Sentence(3..6).fake::<String>();
    let community_id = CommunityDbService::get_profile_community_id(&creator_id);

    let create_response = server
        .post("/api/discussions")
        .add_header("Cookie", format!("jwt={}", creator_token))
        .json(&CreateDiscussion {
            community_id: community_id.to_raw(),
            title: title.clone(),
            image_uri: None,
            chat_user_ids: Some(participant_ids),
            private_discussion_users_final: true,
        })
        .add_header("Accept", "application/json")
        .await;

    create_response.assert_status_success();
    let result = create_response.json::<Discussion>();

    CreatePrivateDiscussionResponse {
        id: result.id.unwrap(),
        title,
        participants: result.private_discussion_user_ids.unwrap(),
    }
}
