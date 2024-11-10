use sb_middleware::ctx::Ctx;
use sb_middleware::db;
use sb_middleware::mw_ctx::{create_ctx_state, CtxState};
use sb_user_auth::routes::register_routes::{register_user, RegisterInput};
use sb_user_auth::routes::webauthn::webauthn_routes::create_webauth_config;
use axum_test::{TestServer, TestServerConfig};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use axum::{middleware, Router};
use axum_htmx::AutoVaryLayer;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;
use tower_cookies::CookieManagerLayer;
use tower_http::services::ServeDir;
use crate::routes::{community_routes, discussion_routes, discussion_topic_routes, post_routes, profile_routes, reply_routes, stripe_routes};
use sb_middleware::{mw_ctx, mw_req_logger};
use sb_user_auth::entity::access_right_entity::AccessRightDbService;
use sb_user_auth::entity::access_rule_entity::AccessRuleDbService;
use sb_user_auth::entity::authentication_entity::AuthenticationDbService;
use crate::entity::community_entitiy::CommunityDbService;
use crate::entity::discussion_entitiy::DiscussionDbService;
use crate::entity::discussion_topic_entitiy::DiscussionTopicDbService;
use sb_user_auth::entity::local_user_entity::LocalUserDbService;
use sb_user_auth::entity::notification_entitiy::NotificationDbService;
use sb_user_auth::entity::payment_action_entitiy::JoinActionDbService;
use crate::entity::post_entitiy::PostDbService;
use crate::entity::reply_entitiy::ReplyDbService;
use sb_user_auth::routes::*;
use sb_user_auth::routes::webauthn::webauthn_routes;
use sb_user_auth::routes::webauthn::webauthn_routes::WebauthnConfig;
use sb_middleware::error::{AppError, AppResult};
use sb_user_auth::entity::follow_entitiy::FollowDbService;

pub async fn create_test_server() -> (AppResult<TestServer>, CtxState) {
    let db = Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos().to_string());
    let db_start = db::start(db).await;
    if db_start.is_err() {
        panic!("DB ERR={:?}",db_start.err().unwrap());
    }
    runMigrations(db_start.unwrap()).await.expect("migrations run");

    let ctx_state = create_ctx_state("123".to_string(), true, "".to_string() , "".to_string() , "".to_string(), "uploads".to_string());
    let wa_config = create_webauth_config();
    let routes_all = main_router(&ctx_state.clone(), wa_config).await;

    let server = TestServer::new_with_config(
        routes_all,
        TestServerConfig { transport: None, save_cookies: true, expect_success_by_default: false, restrict_requests_with_http_schema: false, default_content_type: None, default_scheme: None })
        .map_err(|e| AppError::Generic {description:format!("server did not start err{}", e)});
    (server, ctx_state)
}

#[derive(Deserialize, Debug)]
struct RegisterResponse {
    pub id: String,
}
pub async fn create_login_test_user(server: &TestServer, username: String) -> (&TestServer, String) {
    let create_user = &server.post("/api/register").json(&RegisterInput { username: username.to_string() , password: "some3242paSs#$".to_string(), email:None, next: None, password1: "some3242paSs#$".to_string() }).await;
    create_user.assert_status_success();
    // dbg!(&create_user);
    // let userId: String = create_user;
    let registered = &create_user.json::<RegisterResponse>();
    // let login_user = &server.post("/api/login").json(&LoginInput { username: username.to_string(), password: "some3242paSs#$".to_string(), next: None }).await;
    // login_user.assert_status_success();

    (server, registered.id.clone())
}


pub async fn create_dev_env(ctx_state: &CtxState, username: String, pass: String) {
    let ctx = &Ctx::new(Ok(username.clone().to_string()), Uuid::new_v4(), false);
    let admin = register_user(&ctx_state._db, &ctx, &RegisterInput { username: username.clone().to_string(), password: pass.clone(), password1: pass.clone(),  email: None, next: None }).await.unwrap();
    let user = register_user(&ctx_state._db, &ctx, &RegisterInput { username: "test".to_string(), password: "test123".to_string(), password1: "test123".to_string(), email: None, next: None }).await.unwrap();

}

pub async fn main_router(ctx_state: &CtxState, wa_config: WebauthnConfig ) -> Router {
    Router::new()
        .nest_service("/assets", ServeDir::new("../src/assets"))
        // No requirements
        // Also behind /api, but no auth requirement on this route
        .merge(init_server_routes::routes(ctx_state.clone()))
        .merge(login_routes::routes(ctx_state.clone()))
        .merge(register_routes::routes(ctx_state.clone()))
        .merge(discussion_routes::routes(ctx_state.clone()))
        .merge(discussion_topic_routes::routes(ctx_state.clone()))
        .merge(community_routes::routes(ctx_state.clone()))
        .merge(access_rule_routes::routes(ctx_state.clone()))
        .merge(post_routes::routes(ctx_state.clone()))
        .merge(reply_routes::routes(ctx_state.clone()))
        .merge(webauthn_routes::routes(ctx_state.clone(), wa_config, "../server_main/src/assets/wasm"))
        .merge(stripe_routes::routes(ctx_state.clone()))
        .merge(join_routes::routes(ctx_state.clone()))
        .merge(profile_routes::routes(ctx_state.clone()))
        // .merge(file_upload_routes::routes(ctx_state.clone(), ctx_state.uploads_dir.as_str()).await)
        .layer(AutoVaryLayer)
        .layer(middleware::map_response(mw_req_logger::mw_req_logger))
        // .layer(middleware::map_response(mw_response_transformer::mw_htmx_transformer))
        /*.layer(middleware::from_fn_with_state(
            ctx_state.clone(),
            mw_ctx::mw_require_login,
        ))*/
        // This is where Ctx gets created, with every new request
        .layer(middleware::from_fn_with_state(
            ctx_state.clone(),
            mw_ctx::mw_ctx_constructor,
        )
        )
        // Layers are executed from bottom up, so CookieManager has to be under ctx_constructor
        .layer(CookieManagerLayer::new())
    // .layer(Extension(ctx_state.clone()))
    // .fallback_service(routes_static());
}

async fn runMigrations(db: Surreal<Db>) -> AppResult<()> {
    let c = Ctx::new(Ok("migrations".parse().unwrap()), Uuid::new_v4(), false);
    // let ts= TicketDbService {db: &db, ctx: &c };
    // ts.mutate_db().await?;

    LocalUserDbService { db: &db, ctx: &c }.mutate_db().await?;
    AuthenticationDbService { db: &db, ctx: &c }.mutate_db().await?;
    DiscussionDbService { db: &db, ctx: &c }.mutate_db().await?;
    DiscussionTopicDbService { db: &db, ctx: &c }.mutate_db().await?;
    PostDbService { db: &db, ctx: &c }.mutate_db().await?;
    ReplyDbService { db: &db, ctx: &c }.mutate_db().await?;
    NotificationDbService { db: &db, ctx: &c }.mutate_db().await?;
    CommunityDbService { db: &db, ctx: &c }.mutate_db().await?;
    AccessRuleDbService { db: &db, ctx: &c }.mutate_db().await?;
    AccessRightDbService { db: &db, ctx: &c }.mutate_db().await?;
    JoinActionDbService { db: &db, ctx: &c }.mutate_db().await?;
    FollowDbService { db: &db, ctx: &c }.mutate_db().await?;

    /*
            // ts.create_ticket(CreateTicketInput{title: "iiiii".parse().unwrap()}).await;
            let vec = ts.list_tickets().await?.unwrap();
            println!("LLLL={}", vec.len());

            // &db.create("tickets")
            // .content(Ticket {
            //     id: None,
            //     creator: "system".parse().unwrap(),
            //     title: "init ticket".parse().unwrap(),
            // })
            // .await?;
        let thi = Thing{id: Id::from(134), tb: "tttt".parse().unwrap() };
        let res= Resource::from(thi);
       db.insert(res);*/


    /*
        let sql = "
        CREATE tickets CONTENT
        {
         creator:'sys',
         title:'tttii'
        };
        SELECT * FROM type::table($table);
        SELECT * FROM type::table($table1);
    ";
        let mut result = db
            .query(sql)
            .bind(("table", "tickets"))
            .bind(("table1", "tttt"))
            .await?;
    // Get the first result from the first query
        let created: Option<Ticket> = result.take(0)?;
    // Get all of the results from the second query
        let tickets: Vec<Ticket> = result.take(1)?;
        let ttt: Vec<Thing> = result.take(2)?;

        println!("created len={} 0={} tttt={}", tickets.len(), created.unwrap().title, ttt.len());

    */


    /*
        let sql = "
        SELECT * FROM ttt;
    ";
        let mut sss = db.query(sql).await?;
        // sss.to_string()
        let qr: Vec<Thing> = sss.take(0);
        println!("TTTTT={}", qr.unwrap());*/
    Ok(())
}
