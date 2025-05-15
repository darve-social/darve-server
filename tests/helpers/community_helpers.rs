use axum_test::TestServer;
use darve_server::{
    entities::community::community_entity,
    middleware::{self, ctx::Ctx, mw_ctx::CtxState},
    routes::community::community_routes,
};
use fake::{faker, Fake};
use surrealdb::sql::Thing;
use uuid::Uuid;
pub struct CreateFakeCommunityResponse {
    pub id: String,
    pub name: String,
    pub profile_discussion: Thing,
}

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

    let ctx = Ctx::new(Ok(user_ident), Uuid::new_v4(), false);

    let community_db_service = CommunityDbService {
        db: &ctx_state._db,
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
        profile_discussion: community.profile_discussion.clone().unwrap(),
    }
}
