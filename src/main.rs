use std::net::{Ipv4Addr, SocketAddr};

use config::AppConfig;
use database::client::{Database, DbConfig};
use middleware::error::AppResult;
use middleware::mw_ctx::{self};
use routes::user_auth::webauthn::webauthn_routes::{self};
use tokio;

pub mod config;
pub mod database;
pub mod entities;
pub mod init;
pub mod interfaces;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod services;
pub mod utils;

#[tokio::main]
async fn main() -> AppResult<()> {
    tracing_subscriber::fmt::init();

    let config = AppConfig::from_env();

    let db = Database::connect(DbConfig {
        url: &config.db_url,
        database: &config.db_database,
        namespace: &config.db_namespace,
        password: config.db_password.as_deref(),
        username: config.db_username.as_deref(),
    })
    .await;

    let ctx_state = mw_ctx::create_ctx_state(db.client.clone(), &config).await;

    init::run_migrations(db.client.clone()).await?;
    init::create_default_profiles(&ctx_state, &config.init_server_password.as_str()).await;

    let wa_config = webauthn_routes::create_webauth_config();
    let routes_all = init::main_router(&ctx_state, wa_config).await;

    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8080));
    println!("->> LISTENING on {addr}\n");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    axum::serve(listener, routes_all.into_make_service())
        .await
        .unwrap();

    Ok(())
}
