use tokio::sync::broadcast::Sender;

use crate::{
    entities::user_auth::{
        follow_entity::FollowDbService,
        local_user_entity::LocalUser,
        user_notification_entity::{UserNotificationDbService, UserNotificationEvent},
    },
    middleware::{ctx::Ctx, db, error::CtxResult, mw_ctx::AppEvent},
};

use surrealdb::sql::Thing;

pub struct NotificationService<'a> {
    follow_repository: FollowDbService<'a>,
    notification_repository: UserNotificationDbService<'a>,
    event_sender: &'a Sender<AppEvent>,
}

impl<'a> NotificationService<'a> {
    pub fn new(
        db: &'a db::Db,
        ctx: &'a Ctx,
        event_sender: &'a Sender<AppEvent>,
    ) -> NotificationService<'a> {
        NotificationService {
            follow_repository: FollowDbService { db, ctx },
            notification_repository: UserNotificationDbService { db, ctx },
            event_sender,
        }
    }

    pub async fn on_like(
        &self,
        user_id: &Thing,
        participators: Vec<Thing>,
        post_id: Thing,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();
        let receivers = participators.iter().map(|id| id.to_raw()).collect();

        let event = UserNotificationEvent::UserLikePost {
            user_id: user_id.clone(),
            post_id,
        };
        self.notification_repository
            .notify_users(participators, &event, "")
            .await?;

        let _ = self.event_sender.send(AppEvent {
            user_id: user_id_str,
            content: None,
            event: event,
            receivers,
        });

        Ok(())
    }

    pub async fn on_follow(
        &self,
        user: &LocalUser,
        follow_username: String,
        participators: Vec<Thing>,
    ) -> CtxResult<()> {
        let user_id_str = user.id.as_ref().unwrap().to_raw();
        let receivers = participators.iter().map(|id| id.to_raw()).collect();

        let event = UserNotificationEvent::UserFollowAdded {
            username: user.username.clone(),
            follows_username: follow_username,
        };

        self.notification_repository
            .notify_users(participators, &event, "")
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
            content: None,
            event,
        });

        Ok(())
    }

    pub async fn on_deliver_task(
        &self,
        user_id: &Thing,
        task_id: Thing,
        deliverable: Thing,
        participators: &Vec<Thing>,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();
        let receivers = participators.iter().map(|id| id.to_raw()).collect();

        let event = UserNotificationEvent::UserTaskRequestDelivered {
            task_id,
            deliverable,
            delivered_by: user_id.clone(),
        };

        self.notification_repository
            .notify_users(participators.clone(), &event, "")
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
            content: None,
            event: event,
        });

        Ok(())
    }

    pub async fn on_update_balance(
        &self,
        user_id: &Thing,
        participators: &Vec<Thing>,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();
        let receivers = participators.iter().map(|id| id.to_raw()).collect();

        let event = UserNotificationEvent::UserBalanceUpdate;

        self.notification_repository
            .notify_users(participators.clone(), &event, "")
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
            content: None,
            event: event,
        });

        Ok(())
    }

    pub async fn on_chat_message(
        &self,
        user_id: &Thing,
        participators: &Vec<Thing>,
        content: &String,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();
        let receivers = participators.iter().map(|id| id.to_raw()).collect();

        let event = UserNotificationEvent::UserChatMessage;
        self.notification_repository
            .notify_users(participators.clone(), &event, &content)
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
            content: Some(content.to_owned()),
            event,
        });

        Ok(())
    }

    pub async fn on_community_post(&self, user_id: &Thing, content: &String) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();

        let follower_ids: Vec<Thing> = self
            .follow_repository
            .user_follower_ids(user_id.clone())
            .await?;

        let receivers = follower_ids.iter().map(|id| id.to_raw()).collect();

        let event = UserNotificationEvent::UserCommunityPost;

        self.notification_repository
            .notify_users(follower_ids.clone(), &event, &content)
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
            content: Some(content.to_owned()),
            event,
        });

        Ok(())
    }

    pub async fn on_created_task(
        &self,
        user_id: &Thing,
        task_id: &Thing,
        to_user: &Thing,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();

        let follower_ids: Vec<Thing> = self
            .follow_repository
            .user_follower_ids(user_id.clone())
            .await?;

        let event = UserNotificationEvent::UserTaskRequestCreated {
            task_id: task_id.clone(),
            from_user: user_id.clone(),
            to_user: to_user.clone(),
        };

        self.notification_repository
            .notify_users(follower_ids.clone(), &event, "")
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers: follower_ids.iter().map(|id| id.to_raw()).collect(),
            user_id: user_id_str,
            content: None,
            event,
        });

        Ok(())
    }

    pub async fn on_received_task(
        &self,
        user_id: &Thing,
        task_id: &Thing,
        to_user: &Thing,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();

        let follower_ids: Vec<Thing> = self
            .follow_repository
            .user_follower_ids(to_user.clone())
            .await?;

        let event = UserNotificationEvent::UserTaskRequestReceived {
            task_id: task_id.clone(),
            from_user: user_id.clone(),
            to_user: to_user.clone(),
        };

        self.notification_repository
            .notify_users(follower_ids.clone(), &event, "")
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers: follower_ids.iter().map(|id| id.to_raw()).collect(),
            user_id: user_id_str,
            content: None,
            event,
        });

        Ok(())
    }
}
