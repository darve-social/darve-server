use std::sync::Arc;

use crate::database::repositories::access::AccessRepository;
use crate::database::repositories::delivery_result::DeliveryResultRepository;
use crate::database::repositories::discussion_user::DiscussionUserRepository;
use crate::database::repositories::editor_tags::EditorTagsRepository;
use crate::database::repositories::like::LikesRepository;
use crate::database::repositories::post_user::PostUserRepository;
use crate::database::repositories::reply::RepliesRepository;
use crate::database::repositories::task_donors::TaskDonorsRepository;
use crate::database::repositories::task_participants::TaskParticipantsRepository;
use crate::database::repositories::user_nicknames::NicknamesRepository;
use crate::database::repositories::user_notifications::UserNotificationsRepository;
use crate::database::repositories::verification_code_repo::VERIFICATION_CODE_TABLE_NAME;
use crate::database::repository_impl::Repository;
use crate::database::repository_traits::RepositoryConn;
use crate::entities::verification_code::VerificationCodeEntity;
use crate::middleware::error::AppError;
use surrealdb::engine::any::{connect, Any};
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;

use super::repositories::tags::TagsRepository;

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
    pub tags: TagsRepository,
    pub replies: RepliesRepository,
    pub likes: LikesRepository,
    pub access: AccessRepository,
    pub post_users: PostUserRepository,
    pub discussion_users: DiscussionUserRepository,
    pub nicknames: NicknamesRepository,
    pub editor_tags: EditorTagsRepository,
    pub delivery_result: DeliveryResultRepository,
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
            tags: TagsRepository::new(client.clone()),
            replies: RepliesRepository::new(client.clone()),
            likes: LikesRepository::new(client.clone()),
            access: AccessRepository::new(client.clone()),
            post_users: PostUserRepository::new(client.clone()),
            nicknames: NicknamesRepository::new(client.clone()),
            editor_tags: EditorTagsRepository::new(client.clone()),
            delivery_result: DeliveryResultRepository::new(client.clone()),
            discussion_users: DiscussionUserRepository::new(client),
        }
    }

    pub async fn run_migrations(&self) -> Result<(), AppError> {
        self.verification_code.mutate_db().await?;
        self.user_notifications.mutate_db().await?;
        self.task_donors.mutate_db().await?;
        self.task_participants.mutate_db().await?;
        self.tags.mutate_db().await?;
        self.replies.mutate_db().await?;
        self.likes.mutate_db().await?;
        self.access.mutate_db().await?;
        self.post_users.mutate_db().await?;
        self.discussion_users.mutate_db().await?;
        self.nicknames.mutate_db().await?;
        self.editor_tags.mutate_db().await?;
        self.delivery_result.mutate_db().await?;
        Ok(())
    }
}
