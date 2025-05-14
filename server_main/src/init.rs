use sb_community::entity::community_entitiy::{Community, CommunityDbService};
use sb_middleware::{
    ctx::Ctx,
    db,
    utils::{db_utils::UsernameIdent, string_utils::get_string_thing},
};
use sb_user_auth::entity::{
    authentication_entity::AuthType,
    local_user_entity::{LocalUser, LocalUserDbService},
};
use uuid::Uuid;

async fn create_profile<'a>(
    username: &str,
    password: &str,
    user_service: &'a LocalUserDbService<'a>,
    community_service: &'a CommunityDbService<'a>,
) {
    let is_user = user_service
        .exists(UsernameIdent(username.to_string()).into())
        .await
        .unwrap_or_default()
        .is_some();

    if is_user {
        return;
    };

    let user_id = user_service
        .create(
            LocalUser::default(username.to_string()),
            AuthType::PASSWORD(Some(password.to_string())),
        )
        .await
        .expect("User could not be created");

    let community = Community::new_user_community(&get_string_thing(user_id).expect("is user ident"));

    let _ = community_service
        .create_update(community)
        .await
        .expect("Community could not be created");
}

pub async fn create_default_profiles(db: db::Db, password: &str) {
    let c = Ctx::new(
        Ok("create_drave_profiles".parse().unwrap()),
        Uuid::new_v4(),
        false,
    );

    let user_service = LocalUserDbService { db: &db, ctx: &c };
    let community_service = CommunityDbService { db: &db, ctx: &c };

    let _ = create_profile(
        "darve-starter",
        password,
        &user_service,
        &community_service,
    )
    .await;

    let _ = create_profile(
        "darve-super",
        password,
        &user_service,
        &community_service,
    )
    .await;
}
