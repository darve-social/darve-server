use std::sync::Arc;

use crate::database::repositories::like::LikesRepository;
use crate::database::repositories::reply::RepliesRepository;
use crate::database::repositories::task_donors::TaskDonorsRepository;
use crate::database::repositories::task_participants::TaskParticipantsRepository;
use crate::database::repositories::task_relates::TaskRelatesRepository;
use crate::database::repositories::user_notifications::UserNotificationsRepository;
use crate::database::repositories::verification_code_repo::VERIFICATION_CODE_TABLE_NAME;
use crate::database::repository_impl::Repository;
use crate::database::repository_traits::RepositoryConn;
use crate::entities::verification_code::VerificationCodeEntity;
use crate::middleware::error::AppError;
use surrealdb::engine::any::{connect, Any};
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;

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
    pub verification_code: Repository<VerificationCodeEntity>,
    pub user_notifications: UserNotificationsRepository,
    pub task_donors: TaskDonorsRepository,
    pub task_participants: TaskParticipantsRepository,
    pub task_relates: TaskRelatesRepository,
    pub replies: RepliesRepository,
    pub likes: LikesRepository,
}

impl Database {
    pub async fn connect(config: DbConfig<'_>) -> Self {
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

        let client = Arc::new(conn);

        Self {
            client: client.clone(),
            verification_code: Repository::<VerificationCodeEntity>::new(
                client.clone(),
                VERIFICATION_CODE_TABLE_NAME.to_string(),
            ),
            user_notifications: UserNotificationsRepository::new(client.clone()),
            task_donors: TaskDonorsRepository::new(client.clone()),
            task_participants: TaskParticipantsRepository::new(client.clone()),
            task_relates: TaskRelatesRepository::new(client.clone()),
            replies: RepliesRepository::new(client.clone()),
            likes: LikesRepository::new(client),
        }
    }

    pub async fn run_migrations(&self) -> Result<(), AppError> {
        self.verification_code.mutate_db().await?;
        self.user_notifications.mutate_db().await?;
        self.task_donors.mutate_db().await?;
        self.task_participants.mutate_db().await?;
        self.task_relates.mutate_db().await?;
        self.replies.mutate_db().await?;
        self.likes.mutate_db().await?;
        Ok(())
    }
}
