use surrealdb::engine::any::{connect, Any};
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;
use tracing::info;

pub type Db = Surreal<Any>;

#[derive(Debug)]
pub struct DbConfig<'a> {
    pub url: &'a str,
    pub database: &'a str,
    pub namespace: &'a str,
    pub username: Option<&'a str>,
    pub password: Option<&'a str>,
}

#[derive(Debug)]
pub struct Database {
    pub client: Surreal<Any>,
}

impl Database {
    pub async fn connect(config: DbConfig<'_>) -> Self {
        info!("->> connecting DB config = {:?}", config);
        let conn = connect(config.url).await.expect("");

        match (config.password, config.username) {
            (Some(password), Some(username)) => {
                conn.signin(Root { username, password }).await.expect("");
            }
            _ => {}
        }

        conn.use_ns(config.namespace)
            .use_db(config.database)
            .await
            .expect("");

        let version = conn.version().await.expect("");
        info!("->> connected DB version: {version}");
        Self { client: conn }
    }
}
