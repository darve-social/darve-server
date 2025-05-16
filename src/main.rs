use std::net::{Ipv4Addr, SocketAddr};

use chrono::Duration;
use dotenv::dotenv;
use middleware::db::{self, DBConfig};
use middleware::error::AppResult;
use middleware::mw_ctx::{self};
use routes::user_auth::webauthn::webauthn_routes::{self};
use tokio;

pub mod database;
pub mod entities;
pub mod init;
pub mod middleware;
pub mod routes;
pub mod services;
pub mod utils;

#[tokio::main]
async fn main() -> AppResult<()> {
    dotenv().ok();
    let is_dev = std::env::var("DEVELOPMENT")
        .expect("set DEVELOPMENT env var")
        .eq("true");
    let init_server_password = std::env::var("START_PASSWORD").expect("password to start request");
    let stripe_secret_key =
        std::env::var("STRIPE_SECRET_KEY").expect("Missing STRIPE_SECRET_KEY in env");
    let stripe_wh_secret =
        std::env::var("STRIPE_WEBHOOK_SECRET").expect("Missing STRIPE_WEBHOOK_SECRET in env");
    let stripe_platform_account =
        std::env::var("STRIPE_PLATFORM_ACCOUNT").expect("Missing STRIPE_PLATFORM_ACCOUNT in env");
    let uploads_dir = std::env::var("UPLOADS_DIRECTORY").unwrap_or("uploads".to_string());
    println!("uploads dir = {uploads_dir}");
    let upload_file_size_max_mb: u64 = std::env::var("UPLOAD_MAX_SIZE_MB")
        .unwrap_or("15".to_string())
        .parse()
        .expect("to be number");
    println!("uploads max mb = {upload_file_size_max_mb}");
    let jwt_secret = std::env::var("JWT_SECRET").expect("Missing JWT_SECRET in env");
    let jwt_duration = Duration::days(7);

    let _ = utils::dir_utils::ensure_dir_exists(&uploads_dir);

    let db = db::start(DBConfig::from_env()).await?;

    init::run_migrations(db.clone()).await?;
    init::create_default_profiles(db.clone(), init_server_password.as_str()).await;

    let ctx_state = mw_ctx::create_ctx_state(
        db::DB.clone(),
        init_server_password,
        is_dev,
        jwt_secret,
        jwt_duration,
        stripe_secret_key,
        stripe_wh_secret,
        stripe_platform_account,
        uploads_dir,
        upload_file_size_max_mb,
    );
    let wa_config = webauthn_routes::create_webauth_config();
    let routes_all = init::main_router(&ctx_state, wa_config).await;

    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8080));
    println!("->> LISTENING on {addr}\n");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    let _ = init::create_default_data_for_dev(&ctx_state).await;

    axum::serve(listener, routes_all.into_make_service())
        .await
        .unwrap();

    // fallback fs
    // fn routes_static() -> Router {
    //     Router::new().nest_service("/", get_service(ServeDir::new("./")))
    // }

    Ok(())
}
