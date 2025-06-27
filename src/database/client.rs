use std::sync::Arc;

use surrealdb::engine::any::{connect, Any};
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;
use tracing::info;

use crate::database::repositories::user_notifications::UserNotificationsRepository;
use crate::database::repositories::verification_code::VerificationCodeRepository;
use crate::middleware::error::AppError;

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
    pub client: Arc<Surreal<Any>>,
    pub verification_code: VerificationCodeRepository,
    pub user_notifications: UserNotificationsRepository,
}

impl Database {
    pub async fn connect(config: DbConfig<'_>) -> Self {
        info!("->> connecting DB config = {:?}", config);
        let conn = connect(config.url)
            .await
            .expect("Failed to connect to SurrealDB");

        match (config.password, config.username) {
            (Some(password), Some(username)) => {
                conn.signin(Root { username, password })
                    .await
                    .expect("Failed to sign in to SurrealDB");
            }
            _ => {}
        }

        conn.use_ns(config.namespace)
            .use_db(config.database)
            .await
            .expect("Failed to select namespace and database");

        let version = conn
            .version()
            .await
            .expect("Failed to get SurrealDB version");

        info!("->> connected DB version: {version}");

        let client = Arc::new(conn);

        Self {
            client: client.clone(),
            verification_code: VerificationCodeRepository::new(client.clone()),
            user_notifications: UserNotificationsRepository::new(client.clone()),
        }
    }

    pub async fn run_migrations(&self) -> Result<(), AppError> {
        self.verification_code.mutate_db().await?;
        self.user_notifications.mutate_db().await?;

        Ok(())
    }
}
