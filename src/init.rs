use std::time::Duration;

use crate::{
    entities::{
        self,
        community::community_entity::{Community, CommunityDbService},
        user_auth::{
            authentication_entity::AuthType,
            local_user_entity::{LocalUser, LocalUserDbService},
        },
    },
    middleware::{
        ctx::Ctx,
        db,
        error::{AppError, AppResult, CtxResult},
        mw_ctx::{self, CtxState},
        utils::{
            db_utils::{IdentIdName, UsernameIdent},
            string_utils::get_string_thing,
        },
    },
    routes::{
        self,
        user_auth::register_routes::{register_user, RegisterInput},
        wallet::{wallet_endowment_routes, wallet_routes},
    },
};
use axum::{
    body::Body,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use axum_htmx::AutoVaryLayer;
use entities::community::discussion_entity::DiscussionDbService;
use entities::community::discussion_notification_entity::DiscussionNotificationDbService;
use entities::community::discussion_topic_entity::DiscussionTopicDbService;
use entities::community::post_entity::PostDbService;
use entities::community::post_stream_entity::PostStreamDbService;
use entities::community::reply_entity::ReplyDbService;
use entities::task::task_deliverable_entity::TaskDeliverableDbService;
use entities::task::task_request_entity::TaskRequestDbService;
use entities::task::task_request_participation_entity::TaskParticipationDbService;
use entities::user_auth::access_gain_action_entity::AccessGainActionDbService;
use entities::user_auth::access_right_entity::AccessRightDbService;
use entities::user_auth::access_rule_entity::AccessRuleDbService;
use entities::user_auth::authentication_entity::AuthenticationDbService;
use entities::user_auth::follow_entity::FollowDbService;
use entities::user_auth::user_notification_entity::UserNotificationDbService;
use entities::wallet::currency_transaction_entity::CurrencyTransactionDbService;
use entities::wallet::lock_transaction_entity::LockTransactionDbService;
use entities::wallet::wallet_entity::WalletDbService;
use reqwest::{header::USER_AGENT, Client, StatusCode};
use routes::community::{
    community_routes, discussion_routes, discussion_topic_routes, post_routes, profile_routes,
    reply_routes, stripe_routes,
};
use routes::task::task_request_routes;
use routes::user_auth::webauthn::webauthn_routes::{self, WebauthnConfig};
use routes::user_auth::{
    access_gain_action_routes, access_rule_routes, follow_routes, init_server_routes, login_routes,
    register_routes, user_notification_routes,
};
use tower_cookies::CookieManagerLayer;
use tower_http::services::ServeDir;
use uuid::Uuid;

use axum::http;
use http::Request;
use tower_http::{classify::ServerErrorsFailureClass, trace::TraceLayer};
use tracing::{info, Span};

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
            AuthType::PASSWORD(Some(password.to_string()), None),
        )
        .await
        .expect("User could not be created");

    let community =
        Community::new_user_community(&get_string_thing(user_id).expect("is user ident"));

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

    let _ = create_profile("darve-starter", password, &user_service, &community_service).await;

    let _ = create_profile("darve-super", password, &user_service, &community_service).await;
}

pub async fn run_migrations(db: db::Db) -> AppResult<()> {
    let c = Ctx::new(Ok("migrations".parse().unwrap()), Uuid::new_v4(), false);
    // let ts= TicketDbService {db: &db, ctx: &c };
    // ts.mutate_db().await?;

    LocalUserDbService { db: &db, ctx: &c }.mutate_db().await?;
    AuthenticationDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    DiscussionDbService { db: &db, ctx: &c }.mutate_db().await?;
    DiscussionTopicDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    PostDbService { db: &db, ctx: &c }.mutate_db().await?;
    ReplyDbService { db: &db, ctx: &c }.mutate_db().await?;
    DiscussionNotificationDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    CommunityDbService { db: &db, ctx: &c }.mutate_db().await?;
    AccessRuleDbService { db: &db, ctx: &c }.mutate_db().await?;
    AccessRightDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    AccessGainActionDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    FollowDbService { db: &db, ctx: &c }.mutate_db().await?;
    TaskRequestDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    TaskParticipationDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    TaskDeliverableDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    WalletDbService { db: &db, ctx: &c }.mutate_db().await?;
    CurrencyTransactionDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    LockTransactionDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    PostStreamDbService { db: &db, ctx: &c }.mutate_db().await?;
    UserNotificationDbService { db: &db, ctx: &c }
        .mutate_db()
        .await?;
    Ok(())
}

pub async fn main_router(ctx_state: &CtxState, wa_config: WebauthnConfig) -> Router {
    Router::new()
        .route("/hc", get(get_hc))
        .nest_service("/assets", ServeDir::new("assets"))
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
        .merge(webauthn_routes::routes(
            ctx_state.clone(),
            wa_config,
            "assets/wasm",
        ))
        .merge(stripe_routes::routes(ctx_state.clone()))
        .merge(access_gain_action_routes::routes(ctx_state.clone()))
        .merge(profile_routes::routes(ctx_state.clone()))
        .merge(task_request_routes::routes(ctx_state.clone()))
        .merge(follow_routes::routes(ctx_state.clone()))
        .merge(user_notification_routes::routes(ctx_state.clone()))
        .merge(wallet_routes::routes(ctx_state.clone()))
        .merge(wallet_endowment_routes::routes(ctx_state.clone()))
        // .merge(file_upload_routes::routes(ctx_state.clone(), ctx_state.uploads_dir.as_str()).await)
        .layer(AutoVaryLayer)
        // .layer(axum::middleware::map_response(mw_req_logger))
        // .layer(middleware::map_response(mw_response_transformer::mw_htmx_transformer))
        /*.layer(middleware::from_fn_with_state(
            ctx_state.clone(),
            mw_ctx::mw_require_login,
        ))*/
        // This is where Ctx gets created, with every new request
        .layer(axum::middleware::from_fn_with_state(
            ctx_state.clone(),
            mw_ctx::mw_ctx_constructor,
        ))
        .layer(
            TraceLayer::new_for_http()
                .on_request(|request: &Request<Body>, _: &Span| {
                    let user_agent = request
                        .headers()
                        .get(USER_AGENT)
                        .map(|d| format!("{:?}", d))
                        .unwrap_or("None".to_string());

                    let ctx = request.extensions().get::<Ctx>().cloned();
                    let (req_id, user_id) = match ctx {
                        Some(v) => (v.req_id().to_string(), format!("{:?}", v.user_id())),
                        None => ("None".into(), "None".into()),
                    };

                    info!(
                        request_id = %req_id,
                        user_id = %user_id,
                        method = %request.method(),
                        uri = %request.uri().path(),
                        user_agent = &user_agent,
                        "Request"
                    );
                })
                .on_response(|response: &Response<Body>, latency: Duration, _: &Span| {
                    let status = response.status();
                    let error = response
                        .extensions()
                        .get::<AppError>()
                        .map(|e| format!("{e:?}"));

                    info!(
                        status = %status,
                        latency_ms = %latency.as_millis(),
                        error = ?error,
                        "Response"
                    );
                })
                .on_failure(
                    |error: ServerErrorsFailureClass, _: Duration, _span: &Span| {
                        tracing::debug!("something went wrong {:?}", error)
                    },
                ),
        )
        // Layers are executed from bottom up, so CookieManager has to be under ctx_constructor
        .layer(CookieManagerLayer::new())
    // .layer(Extension(ctx_state.clone()))
    // .fallback_service(routes_static());
}

async fn get_hc() -> Response {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    (StatusCode::OK, format!("v{}", VERSION)).into_response()
}

pub async fn create_default_data_for_dev(ctx_state: &CtxState) {
    if !ctx_state.is_development {
        return;
    };

    let username = "userrr".to_string();
    let password = "password".to_string();
    let email = "dynamite@myheroacademia.io".to_string();
    let bio = "💥 Hero-in-training with explosive ambition to be #1! 💣".to_string();
    let full_name = "Katsuki Bakugo".to_string();
    let image_uri =
        "https://qph.cf2.quoracdn.net/main-qimg-64a32df103bc8fb7b2fc495553a5fc0a-lq".to_string();
    let user_ids = create_dev_env(
        &ctx_state.clone(),
        username.clone(),
        password.clone(),
        Some(email.clone()),
        Some(bio.clone()),
        Some(image_uri.clone()),
        Some(full_name.clone()),
    )
    .await
    .expect("create_dev_env");

    tokio::task::spawn(async move {
        for user_id in user_ids {
            let endow_url = format!("http://localhost:8080/test/api/endow/{}/100", user_id);
            let endowed = Client::new().get(endow_url.clone()).send().await;
            if let Err(err) = endowed {
                println!("Endow test user error: {}", err);
            } else {
                println!("endowed user: {}", endow_url);
            }
        }
    });

    let _ = open::that(format!(
        "http://localhost:8080/login?u={username}&p={password}"
    ));
}

pub async fn create_dev_env(
    ctx_state: &CtxState,
    username: String,
    pass: String,
    email: Option<String>,
    bio: Option<String>,
    image_uri: Option<String>,
    full_name: Option<String>,
) -> CtxResult<Vec<String>> {
    let ctx = &Ctx::new(Ok(username.clone().to_string()), Uuid::new_v4(), false);
    let user_ser = LocalUserDbService {
        db: &ctx_state._db,
        ctx,
    };

    let exists = user_ser
        .get(IdentIdName::ColumnIdent {
            column: "username".to_string(),
            val: "test0".to_string(),
            rec: false,
        })
        .await;

    if exists.is_ok() {
        return Ok(vec![]);
    }

    let hardcoded_bios =
        [
            ("💥 Hero-in-training with explosive ambition to be #0! 💣", "https://static0.gamerantimages.com/wordpress/wp-content/uploads/2023/02/shigaraki-face.jpg"),
            ("🥇 Champ-in-training with explosive ambition to be #1! 💣", "https://fanboydestroy.com/wp-content/uploads/2019/04/ary-and-the-secret-of-seasons-super-resolution-2019.03.22-11.55.42.73.png"),
            ("‼️ QA-in-training with explosive ambition to be #2! 💣", "https://static0.gamerantimages.com/wordpress/wp-content/uploads/2022/07/Genshin-Impact-Sumeru-region.jpg"),
             ("👾 BOT-in-training with explosive ambition to be #3! 💣", "https://static0.gamerantimages.com/wordpress/wp-content/uploads/2023/10/cocoon-container-creature.jpg"),

        ];

    let reg_inputs: Vec<RegisterInput> = hardcoded_bios
        .iter()
        .enumerate()
        .map(|i_bio| {
            let username = format!("test{}", i_bio.0);
            RegisterInput {
                username: username.clone(),
                password: "000000".to_string(),
                password1: "000000".to_string(),
                email: Some(format!("{}@email.com", username.as_str())),
                bio: Some(i_bio.1 .0.to_string()),
                full_name: Some(format!("User {username}")),
                image_uri: Some(i_bio.1 .1.to_string()),
                next: None,
            }
        })
        .collect();

    let id0 = register_user(&ctx_state._db, &ctx, &reg_inputs[0])
        .await
        .unwrap()
        .id;
    let id1 = register_user(&ctx_state._db, &ctx, &reg_inputs[1])
        .await
        .unwrap()
        .id;
    let id2 = register_user(&ctx_state._db, &ctx, &reg_inputs[2])
        .await
        .unwrap()
        .id;
    let id3 = register_user(&ctx_state._db, &ctx, &reg_inputs[3])
        .await
        .unwrap()
        .id;

    // create one more user with the input data

    let id4 = register_user(
        &ctx_state._db,
        &ctx,
        &RegisterInput {
            username: username.clone().to_string(),
            password: pass.clone().to_string(),
            password1: pass.clone().to_string(),
            email: email.clone(),
            bio: bio.clone(),
            full_name: full_name.clone(),
            image_uri: image_uri.clone(),
            next: None,
        },
    )
    .await
    .unwrap()
    .id;
    Ok(vec![id0, id1, id2, id3, id4])
}
