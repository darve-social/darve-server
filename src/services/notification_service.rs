use std::collections::HashSet;

use serde_json::json;
use tokio::sync::broadcast::Sender;

use crate::access::base::role::Role;
use crate::database::client::Db;
use crate::entities::community::discussion_entity::{
    DiscussionType, TABLE_NAME as DISC_TABLE_NAME,
};
use crate::entities::community::post_entity::{Post, PostType, TABLE_NAME as POST_TABLE_NAME};
use crate::entities::discussion_user::DiscussionUser;
use crate::entities::task::task_request_entity::{
    TaskParticipantUserView, TaskRequest, TaskRequestType,
};
use crate::entities::user_notification::UserNotificationEvent;
use crate::interfaces::repositories::user_notifications::UserNotificationsInterface;
use crate::middleware::mw_ctx::AppEventMetadata;
use crate::models::view::access::{DiscussionAccessView, PostAccessView, TaskAccessView};
use crate::models::view::post::PostView;
use crate::models::view::reply::ReplyView;
use crate::{
    entities::user_auth::{follow_entity::FollowDbService, local_user_entity::LocalUser},
    middleware::{
        ctx::Ctx,
        error::{AppError, CtxResult},
        mw_ctx::{AppEvent, AppEventType},
    },
};

use surrealdb::sql::Thing;

pub enum OnCreatedTaskView<'a> {
    Post(&'a PostAccessView),
    Disc(&'a DiscussionAccessView),
}

pub struct NotificationService<'a, N>
where
    N: UserNotificationsInterface,
{
    follow_repository: FollowDbService<'a>,
    notification_repository: &'a N,
    event_sender: &'a Sender<AppEvent>,
    ctx: &'a Ctx,
}

impl<'a, N> NotificationService<'a, N>
where
    N: UserNotificationsInterface,
{
    pub fn new(
        db: &'a Db,
        ctx: &'a Ctx,
        event_sender: &'a Sender<AppEvent>,
        notification_repository: &'a N,
    ) -> NotificationService<'a, N> {
        NotificationService {
            follow_repository: FollowDbService { db, ctx },
            notification_repository,
            event_sender,
            ctx,
        }
    }

    pub async fn on_post_like(
        &self,
        user: &LocalUser,
        post: &PostAccessView,
        is_max_likes: bool,
    ) -> CtxResult<()> {
        let current_user_id = user.id.as_ref().unwrap();
        let receiver_things = match post.r#type {
            PostType::Private => post.get_user_ids(),
            _ => match post.discussion.r#type {
                DiscussionType::Private => post.discussion.get_user_ids(),
                DiscussionType::Public => {
                    let mut users = match is_max_likes {
                        true => self.get_follower_ids(current_user_id.clone()).await?,
                        false => vec![],
                    };

                    post.discussion
                        .get_by_role(Role::Owner.to_string().as_str())
                        .into_iter()
                        .for_each(|id| {
                            if !users.contains(&id) {
                                users.push(id);
                            }
                        });

                    users
                }
            },
        };

        let receivers = receiver_things
            .iter()
            .filter_map(|id| {
                if id == current_user_id {
                    None
                } else {
                    Some(id.id.to_raw())
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let user_id_str = current_user_id.id.to_raw();

        let event = self
            .notification_repository
            .create(
                &user_id_str,
                format!("{} liked the post", user.username).as_str(),
                &UserNotificationEvent::UserLikePost.as_str(),
                &receivers,
                Some(json!({
                    "post_id": post.id.to_raw(),
                    "media_links": post.media_links,
                })),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            user_id: user_id_str,
            metadata: None,
            event: AppEventType::UserNotificationEvent(event),
            content: None,
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
        let user_id_str = user.id.as_ref().unwrap().id.to_raw();
        let receivers = participators
            .iter()
            .map(|id| id.id.to_raw())
            .collect::<Vec<String>>();
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                format!("{} started following {}", user.username, follow_username).as_str(),
                UserNotificationEvent::UserFollowAdded.as_str(),
                &receivers,
                Some(json!({
                    "username": user.username.clone(),
                    "follows_username": follow_username
                })),
            )
            .await?;
        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_deliver_task(
        &self,
        user: &LocalUser,
        task_view: &TaskAccessView,
        result: &PostView,
    ) -> CtxResult<()> {
        let user_id = user.id.as_ref().unwrap();

        let mut receiver_things: HashSet<Thing> = HashSet::from_iter(task_view.get_user_ids());

        if let Some(ref post_view) = task_view.post {
            match post_view.r#type {
                PostType::Private => receiver_things.extend(post_view.get_user_ids()),
                _ => match post_view.discussion.r#type {
                    DiscussionType::Private => {
                        receiver_things.extend(post_view.discussion.get_user_ids())
                    }
                    DiscussionType::Public => {
                        receiver_things.extend(self.get_follower_ids(user_id.clone()).await?)
                    }
                },
            }
        } else if let Some(ref disc_view) = task_view.discussion {
            match disc_view.r#type {
                DiscussionType::Private => receiver_things.extend(disc_view.get_user_ids()),
                DiscussionType::Public => {
                    receiver_things.extend(self.get_follower_ids(user_id.clone()).await?)
                }
            }
        }
        let receivers = receiver_things
            .iter()
            .filter_map(|id| {
                if id == user_id {
                    None
                } else {
                    Some(id.id.to_raw())
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let event = self
            .notification_repository
            .create(
                &user_id.id.to_raw(),
                format!("{} delivered the task.", user.username).as_str(),
                UserNotificationEvent::UserTaskRequestDelivered.as_str(),
                &receivers,
                Some({
                    json!({
                        "task_id": task_view.id,
                        "result_post_id": result.id.to_raw(),
                        "result_links": result.media_links,
                        "post_id": task_view.post.as_ref().map(|p| p.id.to_raw()),
                        "discussion_id": task_view.discussion.as_ref().map(|p| p.id.to_raw()),

                    })
                }),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id.id.to_raw(),
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_accepted_task(
        &self,
        user: &LocalUser,
        task_view: &TaskAccessView,
    ) -> CtxResult<()> {
        let user_id = user.id.as_ref().unwrap();

        let mut receiver_things: HashSet<Thing> = HashSet::from_iter(task_view.get_user_ids());

        if let Some(ref post_view) = task_view.post {
            match post_view.r#type {
                PostType::Private => receiver_things.extend(post_view.get_user_ids()),
                _ => match post_view.discussion.r#type {
                    DiscussionType::Private => {
                        receiver_things.extend(post_view.discussion.get_user_ids())
                    }
                    DiscussionType::Public => {
                        receiver_things.extend(self.get_follower_ids(user_id.clone()).await?)
                    }
                },
            }
        } else if let Some(ref disc_view) = task_view.discussion {
            match disc_view.r#type {
                DiscussionType::Private => receiver_things.extend(disc_view.get_user_ids()),
                DiscussionType::Public => {
                    receiver_things.extend(self.get_follower_ids(user_id.clone()).await?)
                }
            }
        }
        let receivers = receiver_things
            .iter()
            .filter_map(|id| {
                if id == user_id {
                    None
                } else {
                    Some(id.id.to_raw())
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let event = self
            .notification_repository
            .create(
                &user_id.id.to_raw(),
                format!("{} accepted the task.", user.username).as_str(),
                UserNotificationEvent::UserTaskRequestAccepted.as_str(),
                &receivers,
                Some({
                    json!({
                        "task_id": task_view.id,
                        "post_id": task_view.post.as_ref().map(|p| p.id.to_raw()),
                        "discussion_id": task_view.discussion.as_ref().map(|p| p.id.to_raw()),
                    })
                }),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id.id.to_raw(),
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_donate_task(
        &self,
        user: &LocalUser,
        task_view: &TaskAccessView,
    ) -> CtxResult<()> {
        let user_id = user.id.as_ref().unwrap();

        let mut receiver_things: HashSet<Thing> = HashSet::new();

        if let Some(ref post_view) = task_view.post {
            match post_view.r#type {
                PostType::Private => receiver_things.extend(post_view.get_user_ids()),
                _ => match post_view.discussion.r#type {
                    DiscussionType::Private => {
                        receiver_things.extend(post_view.discussion.get_user_ids())
                    }
                    DiscussionType::Public => match task_view.r#type {
                        TaskRequestType::Public => {
                            receiver_things.extend(self.get_follower_ids(user_id.clone()).await?);
                            receiver_things.extend(task_view.get_by_role(&Role::Donor.to_string()));
                        }
                        TaskRequestType::Private => {
                            receiver_things.extend(self.get_follower_ids(user_id.clone()).await?);
                            receiver_things.extend(task_view.get_user_ids());
                        }
                    },
                },
            }
        } else if let Some(ref disc_view) = task_view.discussion {
            match disc_view.r#type {
                DiscussionType::Private => receiver_things.extend(disc_view.get_user_ids()),
                DiscussionType::Public => match task_view.r#type {
                    TaskRequestType::Public => {
                        receiver_things.extend(self.get_follower_ids(user_id.clone()).await?);
                        receiver_things.extend(task_view.get_by_role(&Role::Donor.to_string()));
                    }
                    TaskRequestType::Private => {
                        receiver_things.extend(self.get_follower_ids(user_id.clone()).await?);
                        receiver_things.extend(task_view.get_user_ids());
                    }
                },
            }
        }
        let receivers = receiver_things
            .iter()
            .filter_map(|id| {
                if id == user_id {
                    None
                } else {
                    Some(id.id.to_raw())
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let event = self
            .notification_repository
            .create(
                &user_id.id.to_raw(),
                format!("{} donated to the task.", user.username).as_str(),
                UserNotificationEvent::DonateTaskRequest.as_str(),
                &receivers,
                Some({
                    json!({
                        "task_id": task_view.id,
                        "post_id": task_view.post.as_ref().map(|p| p.id.to_raw()),
                        "discussion_id": task_view.discussion.as_ref().map(|p| p.id.to_raw()),
                    })
                }),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id.id.to_raw(),
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_task_reward(
        &self,
        user: &TaskParticipantUserView,
        task_id: &Thing,
        belongs_to: &Thing,
        donors: &Vec<&Thing>,
    ) -> CtxResult<()> {
        let user_id = &user.id;
        let mut receivers = donors.iter().map(|i| i.to_raw()).collect::<Vec<String>>();
        receivers.push(user_id.to_raw());

        let (post_id, discussion_id) = match belongs_to.tb.as_str() {
            DISC_TABLE_NAME => ("".to_string(), belongs_to.to_raw()),
            POST_TABLE_NAME => (belongs_to.to_raw(), "".to_string()),
            _ => ("".to_string(), "".to_string()),
        };

        let event = self
            .notification_repository
            .create(
                &user_id.id.to_raw(),
                format!("{} received reward for the task.", user.username).as_str(),
                UserNotificationEvent::DonateTaskRequest.as_str(),
                &receivers,
                Some({
                    json!({
                        "task_id": task_id.to_raw(),
                        "post_id": post_id,
                        "discussion_id":  discussion_id
                    })
                }),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id.id.to_raw(),
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_update_balance(&self, user_id: &Thing) -> CtxResult<()> {
        let user_id_str = user_id.id.to_raw();
        let receivers = vec![user_id_str.clone()];
        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
            metadata: None,
            content: None,
            event: AppEventType::UpdatedUserBalance,
        });

        Ok(())
    }

    pub async fn on_created_reply(&self, user: &LocalUser, post: &PostAccessView) -> CtxResult<()> {
        let user_id = user.id.as_ref().expect("User id must exists");
        let receiver_things = post.get_by_role(Role::Owner.to_string().as_str());

        let receivers = receiver_things
            .iter()
            .filter_map(|r| {
                if r == user_id {
                    None
                } else {
                    Some(r.id.to_raw())
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let event = self
            .notification_repository
            .create(
                &user_id.id.to_raw(),
                &format!("{} created the reply", user.username),
                UserNotificationEvent::CommentAdded.as_str(),
                &receivers,
                Some({
                    json!({
                        "post_id": post.id,
                    })
                }),
            )
            .await?;
        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id.id.to_raw(),
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_reply_like(&self, user: &LocalUser, post: &PostAccessView) -> CtxResult<()> {
        let user_id = user.id.as_ref().expect("User id must exists");
        let receiver_things = post.get_by_role(Role::Owner.to_string().as_str());

        let receivers = receiver_things
            .iter()
            .filter_map(|r| {
                if r == user_id {
                    None
                } else {
                    Some(r.id.to_raw())
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let event = self
            .notification_repository
            .create(
                &user_id.id.to_raw(),
                &format!("{} liked the reply", user.username),
                UserNotificationEvent::UserLikeComment.as_str(),
                &receivers,
                Some({
                    json!({
                        "post_id": post.id,
                    })
                }),
            )
            .await?;
        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id.id.to_raw(),
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_completed_deposit(&self, user: &Thing) -> CtxResult<()> {
        let receivers = vec![user.id.to_raw()];
        let event = self
            .notification_repository
            .create(
                &user.id.to_raw(),
                "Deposit completed",
                UserNotificationEvent::DepositCompleted.as_str(),
                &receivers,
                None,
            )
            .await?;
        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user.id.to_raw(),
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_completed_withdraw(&self, user: &Thing) -> CtxResult<()> {
        let receivers = vec![user.id.to_raw()];
        let event = self
            .notification_repository
            .create(
                &user.id.to_raw(),
                "Withdraw completed",
                UserNotificationEvent::WithdrawCompleted.as_str(),
                &receivers,
                None,
            )
            .await?;
        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user.id.to_raw(),
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_created_post(
        &self,
        user: &LocalUser,
        post: &Post,
        disc: &DiscussionAccessView,
        post_members: &Vec<Thing>,
    ) -> CtxResult<()> {
        let user_id = user.id.as_ref().unwrap();

        let receiver_things = match post.r#type {
            PostType::Private => post_members,
            _ => match disc.r#type {
                DiscussionType::Private => &disc.get_user_ids(),
                DiscussionType::Public => &self.get_follower_ids(user_id.clone()).await?,
            },
        };

        let receivers = receiver_things
            .iter()
            .filter_map(|id| {
                if id == user_id {
                    None
                } else {
                    Some(id.id.to_raw())
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let event = self
            .notification_repository
            .create(
                &user_id.id.to_raw(),
                &format!("{} created the post", user.username),
                UserNotificationEvent::CreatedPost.as_str(),
                &receivers,
                Some({
                    json!({
                        "post_id": post.id,
                        "discussion_id": post.belongs_to.to_raw()
                    })
                }),
            )
            .await?;
        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id.id.to_raw(),
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_created_task(
        &self,
        user: &LocalUser,
        task: &TaskRequest,
        view: OnCreatedTaskView<'_>,
        participant: Option<&LocalUser>,
        is_current_user_donor: bool,
    ) -> CtxResult<()> {
        let user_id = user.id.as_ref().expect("User id must exists");
        let receiver_things = match view {
            OnCreatedTaskView::Post(view) => match view.r#type {
                PostType::Private => view.get_user_ids(),
                _ => {
                    self.get_receivers_of_disc_access_view(
                        &view.discussion,
                        &task,
                        user_id.clone(),
                        is_current_user_donor,
                        participant.map(|u| u.id.as_ref().expect("User id must exists").clone()),
                    )
                    .await?
                }
            },
            OnCreatedTaskView::Disc(view) => {
                self.get_receivers_of_disc_access_view(
                    &view,
                    &task,
                    user_id.clone(),
                    is_current_user_donor,
                    participant.map(|u| u.id.as_ref().expect("User id must exists").clone()),
                )
                .await?
            }
        };

        let receivers = receiver_things
            .iter()
            .filter_map(|id| {
                if id == user_id {
                    None
                } else {
                    Some(id.id.to_raw())
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let (post_id, discussion_id) = match view {
            OnCreatedTaskView::Post(post_access_view) => (Some(post_access_view.id.to_raw()), None),
            OnCreatedTaskView::Disc(discussion_access_view) => {
                (None, Some(discussion_access_view.id.to_raw()))
            }
        };

        let event = self
            .notification_repository
            .create(
                &user_id.id.to_raw(),
                format!("{} created a task", user.username).as_str(),
                UserNotificationEvent::UserTaskRequestCreated.as_str(),
                &receivers.clone(),
                Some({
                    json!({
                        "task_id": task.id.as_ref().unwrap().to_raw(),
                        "post_id": post_id,
                        "discussion_id": discussion_id
                    })
                }),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            content: None,
            user_id: user_id.id.to_raw(),
            metadata: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_discussion_post_reply(
        &self,
        user_id: &Thing,
        post_id: &Thing,
        discussion_id: &Thing,
        content: &ReplyView,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();

        let follower_ids: Vec<Thing> = self
            .follow_repository
            .user_follower_ids(user_id.clone())
            .await?;

        let metadata = AppEventMetadata {
            discussion_id: Some(discussion_id.clone()),
            post_id: Some(post_id.clone()),
        };
        let _ = self.event_sender.send(AppEvent {
            receivers: follower_ids.iter().map(|id| id.to_raw()).collect(),
            user_id: user_id_str,
            content: Some(serde_json::to_string(&content).map_err(|_| {
                self.ctx.to_ctx_error(AppError::Generic {
                    description: "Reply to json error for notification event".to_string(),
                })
            })?),
            metadata: Some(metadata.clone()),
            event: AppEventType::DiscussionPostReplyAdded,
        });

        Ok(())
    }

    pub async fn on_discussion_post_reply_nr_increased(
        &self,
        user_id: &Thing,
        post_id: &Thing,
        discussion_id: &Thing,
        content: &String,
    ) -> CtxResult<()> {
        let user_id_str = user_id.to_raw();

        let follower_ids: Vec<Thing> = self
            .follow_repository
            .user_follower_ids(user_id.clone())
            .await?;

        let metadata = AppEventMetadata {
            discussion_id: Some(discussion_id.clone()),
            post_id: Some(post_id.clone()),
        };

        let receivers = follower_ids
            .iter()
            .map(|id| id.to_raw())
            .collect::<Vec<String>>();

        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_id_str,
            metadata: Some(metadata),
            content: Some(content.clone()),
            event: AppEventType::DiscussionPostReplyNrIncreased,
        });

        Ok(())
    }

    pub async fn on_discussion_post(&self, user_id: &Thing, post: &PostView) -> CtxResult<()> {
        let receivers = vec![user_id.to_raw()];

        let post_json = serde_json::to_string(&post).map_err(|_| {
            self.ctx.to_ctx_error(AppError::Generic {
                description: "Post to json error for notification event".to_string(),
            })
        })?;

        let metadata = AppEventMetadata {
            discussion_id: Some(post.belongs_to.clone()),
            post_id: Some(post.id.clone()),
        };

        let _ = self.event_sender.send(AppEvent {
            user_id: user_id.to_raw(),
            event: AppEventType::DiscussionPostAdded,
            content: Some(post_json),
            receivers,
            metadata: Some(metadata),
        });

        Ok(())
    }

    pub async fn on_updated_users_discussions(
        &self,
        user_id: &Thing,
        updated_data: &Vec<DiscussionUser>,
    ) -> CtxResult<()> {
        let receivers = updated_data
            .iter()
            .map(|d| d.user.id.to_raw())
            .collect::<Vec<String>>();

        let _ = self.event_sender.send(AppEvent {
            user_id: user_id.id.to_raw(),
            event: AppEventType::UpdateDiscussionsUsers(updated_data.clone()),
            content: None,
            receivers,
            metadata: None,
        });

        Ok(())
    }

    async fn get_receivers_of_disc_access_view(
        &self,
        view: &DiscussionAccessView,
        task: &TaskRequest,
        curernt_user_id: Thing,
        is_current_user_donor: bool,
        participant: Option<Thing>,
    ) -> CtxResult<Vec<Thing>> {
        match view.r#type {
            DiscussionType::Private => Ok(view.get_user_ids()),
            DiscussionType::Public => match task.r#type {
                TaskRequestType::Public => Ok(vec![]),
                TaskRequestType::Private => {
                    let mut users = if is_current_user_donor {
                        self.get_follower_ids(curernt_user_id).await?
                    } else {
                        vec![]
                    };
                    users.push(participant.expect("Participant must exists for private task"));
                    Ok(users)
                }
            },
        }
    }

    async fn get_follower_ids(&self, user: Thing) -> CtxResult<Vec<Thing>> {
        Ok(self.follow_repository.user_follower_ids(user).await?)
    }
}
