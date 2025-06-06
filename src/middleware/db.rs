use crate::middleware::error::AppResult;
use once_cell::sync::Lazy;
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;
use tracing::info;

pub type Db = Surreal<Any>;

pub static DB: Lazy<Db> = Lazy::new(Surreal::init);
const MEM_DB_URL: &str = "mem://";

#[derive(Debug)]
pub struct DBConfig {
    pub namespace: String,
    pub database: String,
    pub password: Option<String>,
    pub username: Option<String>,
    pub url: String,
}

impl DBConfig {
    pub fn from_env() -> Self {
        let namespace = std::env::var("DB_NAMESPACE").unwrap_or("namespace".to_string());
        let database = std::env::var("DB_DATABASE").unwrap_or("database".to_string());
        let password = std::env::var("DB_PASSWORD").ok();
        let username = std::env::var("DB_USERNAME").ok();
        let url = std::env::var("DB_URL").unwrap_or(MEM_DB_URL.to_string());

        Self {
            namespace,
            database,
            password,
            username,
            url,
        }
    }
}

pub async fn start(config: DBConfig) -> AppResult<Db> {
    info!(
        "->> connecting DB {} ns={} db={}",
        config.url.as_str(),
        config.namespace.as_str(),
        config.database.as_str()
    );
    let _conn = DB.connect(config.url.clone()).await?;

    match (
        config.password.as_ref(),
        config.username.as_ref(),
        config.url.as_str(),
    ) {
        (Some(password), Some(username), url) if url != MEM_DB_URL => {
            DB.signin(Root { username, password }).await?;
        }
        _ => {}
    }

    DB.use_ns(config.namespace).use_db(config.database).await?;

    let version = DB.version().await?;
    info!("->> connected DB version: {version}");
    Ok(DB.clone())
}
