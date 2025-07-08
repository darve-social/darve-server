use std::net::{Ipv4Addr, SocketAddr};

use config::AppConfig;
use database::client::{Database, DbConfig};
use middleware::mw_ctx::{self};
use routes::user_auth::webauthn::webauthn_routes::{self};
use sentry::{ClientInitGuard, ClientOptions};

pub mod config;
pub mod database;
pub mod entities;
pub mod init;
pub mod interfaces;
pub mod jobs;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod services;
pub mod utils;

fn main() {
    tracing_subscriber::fmt::init();
    let config = AppConfig::from_env();

    let _guard: Option<ClientInitGuard> = if let Some(ref link) = config.sentry_project_link {
        let options = ClientOptions {
            release: sentry::release_name!(),
            traces_sample_rate: 0.2,
            send_default_pii: true,
            ..Default::default()
        };
        Some(sentry::init((link.as_str(), options)))
    } else {
        None
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async_main(config));
}

async fn async_main(config: AppConfig) {
    let db = Database::connect(DbConfig {
        url: &config.db_url,
        database: &config.db_database,
        namespace: &config.db_namespace,
        password: config.db_password.as_deref(),
        username: config.db_username.as_deref(),
    })
    .await;

    db.run_migrations().await.unwrap();

    let ctx_state = mw_ctx::create_ctx_state(db, &config).await;

    init::run_migrations(&ctx_state.db).await.unwrap();
    init::create_default_profiles(&ctx_state, &config.init_server_password.as_str()).await;

    let wa_config = webauthn_routes::create_webauth_config();
    let routes_all = init::main_router(&ctx_state, wa_config).await;

    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8080));
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    let _task_handle = jobs::task_payment::run(ctx_state.clone()).await;

    axum::serve(listener, routes_all.into_make_service())
        .await
        .unwrap();
}
