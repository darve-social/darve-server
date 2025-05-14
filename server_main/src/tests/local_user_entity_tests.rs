#[cfg(test)]
mod tests {
    use surrealdb::sql::Thing;
    use uuid::Uuid;

    use crate::test_utils::{create_login_test_user, create_test_server};
    use sb_middleware::ctx::Ctx;
    use sb_middleware::utils::db_utils::{IdentIdName, UsernameIdent};
    use sb_middleware::utils::string_utils::get_string_thing;
    use sb_user_auth::entity::local_user_entity::LocalUserDbService;

    #[tokio::test]
    async fn user_query() {
        let (server, ctx_state) = create_test_server().await;
        let username = "usn1ame".to_string();
        let (server, uid) = create_login_test_user(&server, username.clone()).await;

        let db_service = LocalUserDbService {
            db: &ctx_state._db,
            ctx: &Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false),
        };
        let user = db_service.get(UsernameIdent(username.clone()).into()).await;
        let user = user.unwrap();
        assert_eq!(user.username, username.clone());

        let user = db_service
            .get(IdentIdName::Id(
                get_string_thing(uid.clone()).expect("thing"),
            ))
            .await;
        let user = user.unwrap();
        assert_eq!(user.username, username.clone());

        let user = db_service
            .get(IdentIdName::Id(Thing::from(("local_user", "not_existing"))))
            .await;
        assert_eq!(user.is_err(), true);
    }

    #[tokio::test]
    async fn test_exists() {
        let (server, ctx_state) = create_test_server().await;
        let username = "usn1ame".to_string();
        let (server, uid) = create_login_test_user(&server, username.clone()).await;

        let db_service = LocalUserDbService {
            db: &ctx_state._db,
            ctx: &Ctx::new(Ok("user_ident".parse().unwrap()), Uuid::new_v4(), false),
        };
        let user = db_service
            .exists(UsernameIdent(username.clone()).into())
            .await;
        let user_id = user.unwrap().unwrap();
        assert_eq!(user_id, uid);

        let user = db_service
            .exists(UsernameIdent("not_exists".to_string()).into())
            .await;
        let user = user.unwrap();
        assert_eq!(user.is_some(), false);
    }
}
