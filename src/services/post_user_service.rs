use crate::{
    access::post::PostAccess,
    database::{client::Db, repositories::user_notifications::UserNotificationsRepository},
    entities::{
        community::post_entity::{PostDbService, PostUserStatus},
        discussion_user::DiscussionUser,
        user_auth::local_user_entity::LocalUserDbService,
    },
    interfaces::repositories::{
        discussion_user::DiscussionUserRepositoryInterface, post_user::PostUserRepositoryInterface,
    },
    middleware::{
        ctx::Ctx,
        error::{AppError, AppResult},
        mw_ctx::CtxState,
    },
    models::view::access::PostAccessView,
    services::notification_service::NotificationService,
};

pub struct PostUserService<'a, PU, DU>
where
    DU: DiscussionUserRepositoryInterface,
    PU: PostUserRepositoryInterface,
{
    discussion_users: &'a DU,
    post_user_repository: &'a PU,
    posts_repository: PostDbService<'a>,
    users_repository: LocalUserDbService<'a>,
    notification_service: NotificationService<'a, UserNotificationsRepository>,
    db: &'a Db,
}

impl<'a, PU, DU> PostUserService<'a, PU, DU>
where
    DU: DiscussionUserRepositoryInterface,
    PU: PostUserRepositoryInterface,
{
    pub fn new(
        state: &'a CtxState,
        ctx: &'a Ctx,
        post_user_repository: &'a PU,
        discussion_users: &'a DU,
        notification_service: NotificationService<'a, UserNotificationsRepository>,
    ) -> Self {
        Self {
            discussion_users,
            post_user_repository,
            posts_repository: PostDbService {
                db: &state.db.client,
                ctx: ctx,
            },
            users_repository: LocalUserDbService {
                db: &state.db.client,
                ctx: ctx,
            },
            notification_service,
            db: &state.db.client,
        }
    }

    pub async fn deliver(&self, user_id: &str, post_id: &str) -> AppResult<()> {
        let user = self.users_repository.get_by_id(user_id).await?;
        let user_thing = user.id.as_ref().unwrap().clone();
        let post = self
            .posts_repository
            .get_view_by_id::<PostAccessView>(post_id, None)
            .await?;
        let post_access = PostAccess::new(&post);

        if !post_access.can_view(&user) {
            return Err(AppError::Forbidden);
        }

        let status = self
            .post_user_repository
            .get(user_thing, post.id.clone())
            .await?;

        match status {
            Some(value) => match value {
                PostUserStatus::Delivered => Ok(()),
                PostUserStatus::Seen => Err(AppError::Forbidden),
            },
            None => {
                let res = self
                    .post_user_repository
                    .create(user.id.unwrap(), post.id, PostUserStatus::Delivered as u8)
                    .await?;

                Ok(res)
            }
        }
    }

    pub async fn read(&self, user_id: &str, post_id: &str) -> AppResult<()> {
        let user = self.users_repository.get_by_id(user_id).await?;
        let user_thing = user.id.as_ref().unwrap().clone();
        let post = self
            .posts_repository
            .get_view_by_id::<PostAccessView>(post_id, None)
            .await?;
        if !PostAccess::new(&post).can_view(&user) {
            return Err(AppError::Forbidden);
        }
        let status = self
            .post_user_repository
            .get(user_thing.clone(), post.id.clone())
            .await?;

        if status == Some(PostUserStatus::Seen) {
            return Ok(());
        }

        let mut query = self.db.query("BEGIN");

        query = self.post_user_repository.build_upsert_query(
            query,
            user.id.unwrap(),
            post.id.clone(),
            PostUserStatus::Seen as u8,
        );

        query = self.discussion_users.build_decrease_query(
            query,
            &post.discussion.id.id.to_raw(),
            vec![user_id.to_string()],
        );

        query = query.query("COMMIT").query("RETURN $discussion_users;");

        let mut res = query.await?;
        let data = res.take::<Vec<DiscussionUser>>(res.num_statements() - 1)?;
        let _ = self
            .notification_service
            .on_updated_users_discussions(&user_thing, &data)
            .await?;
        Ok(())
    }
}
