use surrealdb::types::RecordId;
use std::collections::HashSet;

use serde_json::json;
use tokio::sync::broadcast::Sender;

use crate::access::base::role::Role;
use crate::database::client::Db;
use crate::database::surrdb_utils::{record_id_key_to_string, record_id_to_raw};
use crate::entities::community::discussion_entity::{
    DiscussionType, TABLE_NAME as DISC_TABLE_NAME,
};
use crate::entities::community::post_entity::{PostType, TABLE_NAME as POST_TABLE_NAME};
use crate::entities::discussion_user::DiscussionUser;
use crate::entities::task_request::TaskRequestEntity;
use crate::entities::task_request::{TaskParticipantUserView, TaskRequestType};
use crate::entities::user_notification::UserNotificationEvent;
use crate::entities::wallet::wallet_entity::CurrencySymbol;
use crate::interfaces::repositories::user_notifications::UserNotificationsInterface;
use crate::middleware::error::AppResult;
use crate::models::view::access::{DiscussionAccessView, PostAccessView, TaskAccessView};
use crate::models::view::post::PostView;
use crate::{
    entities::user_auth::{follow_entity::FollowDbService, local_user_entity::LocalUser},
    middleware::{
        ctx::Ctx,
        error::{AppError, CtxResult},
        mw_ctx::{AppEvent, AppEventType},
    },
};



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
                    Some(record_id_key_to_string(&id.key))
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let user_id_str = record_id_key_to_string(&current_user_id.key);

        let event = self
            .notification_repository
            .create(
                &user_id_str,
                format!("{} liked the post", user.username).as_str(),
                &UserNotificationEvent::UserLikePost.as_str(),
                &receivers,
                Some(json!({
                    "post_id": record_id_to_raw(&post.id),
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

    pub async fn on_follow(&self, user: &LocalUser, follow: &LocalUser) -> CtxResult<()> {
        let user_id_str = record_id_key_to_string(&user.id.as_ref().unwrap().key);
        let receivers = vec![record_id_key_to_string(&follow.id.as_ref().unwrap().key)];
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                format!("{} started following {}", user.username, follow.username).as_str(),
                UserNotificationEvent::UserFollowAdded.as_str(),
                &receivers,
                Some(json!({
                    "username": user.username.clone(),
                    "follows_username": follow.username
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
        result_link: &str,
        result_post: Option<String>,
    ) -> CtxResult<()> {
        let user_id = user.id.as_ref().unwrap();

        let mut receiver_things: HashSet<RecordId> = HashSet::from_iter(task_view.get_user_ids());

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
                    Some(record_id_key_to_string(&id.key))
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let user_id_str = record_id_key_to_string(&user_id.key);
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                format!("{} delivered the task.", user.username).as_str(),
                UserNotificationEvent::UserTaskRequestDelivered.as_str(),
                &receivers,
                Some(json!({
                    "task_id": record_id_to_raw(&task_view.id),
                    "result_post_id": result_post,
                    "result_link": result_link,
                    "post_id": task_view.post.as_ref().map(|p| record_id_to_raw(&p.id)),
                    "discussion_id": task_view.discussion.as_ref().map(|p| record_id_to_raw(&p.id)),

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

    pub async fn on_rejected_task(
        &self,
        user: &LocalUser,
        task_view: &TaskAccessView,
    ) -> CtxResult<()> {
        let user_id = user.id.as_ref().unwrap();

        let mut receiver_things: HashSet<RecordId> = HashSet::from_iter(task_view.get_user_ids());

        if let Some(ref post_view) = task_view.post {
            match post_view.r#type {
                PostType::Private => receiver_things.extend(post_view.get_user_ids()),
                _ => match post_view.discussion.r#type {
                    DiscussionType::Private => {
                        receiver_things.extend(post_view.discussion.get_user_ids())
                    }
                    _ => (),
                },
            }
        } else if let Some(ref disc_view) = task_view.discussion {
            match disc_view.r#type {
                DiscussionType::Private => receiver_things.extend(disc_view.get_user_ids()),
                _ => (),
            }
        }
        let receivers = receiver_things
            .iter()
            .filter_map(|id| {
                if id == user_id {
                    None
                } else {
                    Some(record_id_key_to_string(&id.key))
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let user_id_str = record_id_key_to_string(&user_id.key);
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                format!("{} rejected the task.", user.username).as_str(),
                UserNotificationEvent::UserTaskRequestRejected.as_str(),
                &receivers,
                Some(json!({
                    "task_id": record_id_to_raw(&task_view.id),
                    "post_id": task_view.post.as_ref().map(|p| record_id_to_raw(&p.id)),
                    "discussion_id": task_view.discussion.as_ref().map(|p| record_id_to_raw(&p.id)),
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

    pub async fn on_accepted_task(
        &self,
        user: &LocalUser,
        task_view: &TaskAccessView,
    ) -> CtxResult<()> {
        let user_id = user.id.as_ref().unwrap();

        let mut receiver_things: HashSet<RecordId> = HashSet::from_iter(task_view.get_user_ids());

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
                    Some(record_id_key_to_string(&id.key))
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let user_id_str = record_id_key_to_string(&user_id.key);
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                format!("{} accepted the task.", user.username).as_str(),
                UserNotificationEvent::UserTaskRequestAccepted.as_str(),
                &receivers,
                Some(json!({
                    "task_id": record_id_to_raw(&task_view.id),
                    "post_id": task_view.post.as_ref().map(|p| record_id_to_raw(&p.id)),
                    "discussion_id": task_view.discussion.as_ref().map(|p| record_id_to_raw(&p.id)),
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

    pub async fn on_donate_task(
        &self,
        user: &LocalUser,
        task_view: &TaskAccessView,
        amount: u64,
        _currency: CurrencySymbol,
    ) -> CtxResult<()> {
        let user_id = user.id.as_ref().unwrap();

        let mut receiver_things: HashSet<RecordId> = HashSet::new();

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
                    Some(record_id_key_to_string(&id.key))
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let link = task_view
            .post
            .as_ref()
            .and_then(|p| p.media_links.as_ref())
            .and_then(|links| links.first());

        let user_id_str = record_id_key_to_string(&user_id.key);
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                format!(
                    "{} donated ${} on the task.",
                    user.username,
                    (amount as f64 / 100.0)
                )
                .as_str(),
                UserNotificationEvent::DonateTaskRequest.as_str(),
                &receivers,
                Some(json!({
                    "task_id": record_id_to_raw(&task_view.id),
                    "media_link": link,
                    "post_id": task_view.post.as_ref().map(|p| record_id_to_raw(&p.id)),
                    "discussion_id": task_view.discussion.as_ref().map(|p| record_id_to_raw(&p.id)),
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

    pub async fn on_task_reward(
        &self,
        user: &TaskParticipantUserView,
        task_id: &str,
        belongs_to: &RecordId,
        donors: &Vec<&RecordId>,
    ) -> CtxResult<()> {
        let user_id = &user.id;
        let mut receivers = donors.iter().map(|i| record_id_to_raw(i)).collect::<Vec<String>>();
        receivers.push(record_id_to_raw(user_id));

        let (post_id, discussion_id) = match belongs_to.table.as_str() {
            DISC_TABLE_NAME => ("".to_string(), record_id_to_raw(belongs_to)),
            POST_TABLE_NAME => (record_id_to_raw(belongs_to), "".to_string()),
            _ => ("".to_string(), "".to_string()),
        };

        let user_id_str = record_id_key_to_string(&user_id.key);
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                format!("{} received reward for the task.", user.username).as_str(),
                UserNotificationEvent::DonateTaskRequest.as_str(),
                &receivers,
                Some(json!({
                    "task_id": task_id,
                    "post_id": post_id,
                    "discussion_id":  discussion_id
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

    pub async fn on_update_balance(&self, user_id: &RecordId) -> CtxResult<()> {
        let user_id_str = record_id_key_to_string(&user_id.key);
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
                    Some(record_id_key_to_string(&r.key))
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let user_id_str = record_id_key_to_string(&user_id.key);
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                &format!("{} created the reply", user.username),
                UserNotificationEvent::CommentAdded.as_str(),
                &receivers,
                Some(json!({
                    "post_id": record_id_to_raw(&post.id),
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

    pub async fn on_reply_like(&self, user: &LocalUser, post: &PostAccessView) -> CtxResult<()> {
        let user_id = user.id.as_ref().expect("User id must exists");
        let receiver_things = post.get_by_role(Role::Owner.to_string().as_str());

        let receivers = receiver_things
            .iter()
            .filter_map(|r| {
                if r == user_id {
                    None
                } else {
                    Some(record_id_key_to_string(&r.key))
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let user_id_str = record_id_key_to_string(&user_id.key);
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                &format!("{} liked the reply", user.username),
                UserNotificationEvent::UserLikeComment.as_str(),
                &receivers,
                Some(json!({
                    "post_id": record_id_to_raw(&post.id),
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

    pub async fn on_completed_deposit(&self, user: &RecordId) -> CtxResult<()> {
        let user_key = record_id_key_to_string(&user.key);
        let receivers = vec![user_key.clone()];
        let event = self
            .notification_repository
            .create(
                &user_key,
                "Deposit completed",
                UserNotificationEvent::DepositCompleted.as_str(),
                &receivers,
                None,
            )
            .await?;
        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_key,
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_completed_withdraw(&self, user: &RecordId) -> CtxResult<()> {
        let user_key = record_id_key_to_string(&user.key);
        let receivers = vec![user_key.clone()];
        let event = self
            .notification_repository
            .create(
                &user_key,
                "Withdraw completed",
                UserNotificationEvent::WithdrawCompleted.as_str(),
                &receivers,
                None,
            )
            .await?;
        let _ = self.event_sender.send(AppEvent {
            receivers,
            user_id: user_key,
            metadata: None,
            content: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_created_task(
        &self,
        user: &LocalUser,
        task: &TaskRequestEntity,
        view: OnCreatedTaskView<'_>,
        participants: Vec<&LocalUser>,
        amount: Option<u64>,
    ) -> CtxResult<()> {
        let user_id = user.id.as_ref().expect("User id must exists");
        let is_current_user_donor = amount.unwrap_or(0) > 0;

        let (post_view, discussion_id) = match view {
            OnCreatedTaskView::Post(post_access_view) => (Some(post_access_view), None),
            OnCreatedTaskView::Disc(discussion_access_view) => {
                (None, Some(record_id_to_raw(&discussion_access_view.id)))
            }
        };
        let link = post_view
            .as_ref()
            .and_then(|p| p.media_links.as_ref())
            .and_then(|links| links.first());

        let post_id = post_view.as_ref().map(|p| record_id_to_raw(&p.id));

        let metadata = Some(json!({
            "task_id": &task.id,
            "media_link": link,
            "post_id": post_id,
            "discussion_id": discussion_id
        }));

        let participant_ids = participants
            .iter()
            .map(|u| u.id.as_ref().unwrap())
            .collect::<Vec<&RecordId>>();

        let user_id_str = record_id_key_to_string(&user_id.key);
        if !participant_ids.is_empty() {
            let ids = participant_ids
                .iter()
                .map(|id| record_id_key_to_string(&id.key))
                .collect::<Vec<String>>();

            let amount_meesage = match amount {
                Some(v) if v > 0 => format!("${}", v as f64 / 100.0),
                _ => "a".to_string(),
            };

            let event = self
                .notification_repository
                .create(
                    &user_id_str,
                    format!("{} created {} task for you", user.username, amount_meesage).as_str(),
                    UserNotificationEvent::UserTaskRequestReceived.as_str(),
                    &ids,
                    metadata.clone(),
                )
                .await?;

            let _ = self.event_sender.send(AppEvent {
                receivers: ids,
                content: None,
                user_id: user_id_str.clone(),
                metadata: None,
                event: AppEventType::UserNotificationEvent(event),
            });
        };

        let receiver_things = match view {
            OnCreatedTaskView::Post(view) => match view.r#type {
                PostType::Private => view.get_user_ids(),
                _ => {
                    self.get_receivers_of_disc_access_view(
                        &view.discussion,
                        &task,
                        user_id.clone(),
                        is_current_user_donor,
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
                )
                .await?
            }
        };

        let receivers = receiver_things
            .iter()
            .filter_map(|id| {
                if id == user_id || participant_ids.contains(&id) {
                    None
                } else {
                    Some(record_id_key_to_string(&id.key))
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                format!("{} created a task", user.username).as_str(),
                UserNotificationEvent::UserTaskRequestCreated.as_str(),
                &receivers.clone(),
                metadata,
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            content: None,
            user_id: user_id_str,
            metadata: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    pub async fn on_discussion_post(
        &self,
        user_id: &str,
        receivers: Vec<String>,
        post: &PostView,
    ) -> CtxResult<()> {
        let post_json = serde_json::to_string(&post).map_err(|_| {
            self.ctx.to_ctx_error(AppError::Generic {
                description: "Post to json error for notification event".to_string(),
            })
        })?;

        let _ = self.event_sender.send(AppEvent {
            user_id: user_id.to_string(),
            event: AppEventType::DiscussionPostAdded,
            content: Some(post_json),
            receivers,
            metadata: None,
        });

        Ok(())
    }

    pub async fn on_updated_users_discussions(
        &self,
        user_id: &RecordId,
        updated_data: &Vec<DiscussionUser>,
    ) -> CtxResult<()> {
        let receivers = updated_data
            .iter()
            .map(|d| record_id_key_to_string(&d.user.key))
            .collect::<Vec<String>>();

        let _ = self.event_sender.send(AppEvent {
            user_id: record_id_key_to_string(&user_id.key),
            event: AppEventType::UpdateDiscussionsUsers(updated_data.clone()),
            content: None,
            receivers,
            metadata: None,
        });

        Ok(())
    }

    pub async fn on_created_discussion(
        &self,
        user: &LocalUser,
        discussion_id: &RecordId,
        participators: &Vec<RecordId>,
    ) -> AppResult<()> {
        let user_id = user.id.as_ref().expect("User id must exists");
        let receivers = participators
            .iter()
            .filter_map(|id| {
                if id == user_id {
                    None
                } else {
                    Some(record_id_key_to_string(&id.key))
                }
            })
            .collect::<Vec<String>>();

        if receivers.is_empty() {
            return Ok(());
        }

        let user_id_str = record_id_key_to_string(&user_id.key);
        let event = self
            .notification_repository
            .create(
                &user_id_str,
                format!("{} created the discussion", user.username).as_str(),
                UserNotificationEvent::CreatedDiscussion.as_str(),
                &receivers.clone(),
                Some(json!({
                    "discussion_id": record_id_to_raw(discussion_id)
                })),
            )
            .await?;

        let _ = self.event_sender.send(AppEvent {
            receivers,
            content: None,
            user_id: user_id_str,
            metadata: None,
            event: AppEventType::UserNotificationEvent(event),
        });

        Ok(())
    }

    async fn get_receivers_of_disc_access_view(
        &self,
        view: &DiscussionAccessView,
        task: &TaskRequestEntity,
        current_user_id: RecordId,
        is_current_user_donor: bool,
    ) -> CtxResult<Vec<RecordId>> {
        match view.r#type {
            DiscussionType::Private => Ok(view.get_user_ids()),
            DiscussionType::Public => match task.r#type {
                TaskRequestType::Public => Ok(vec![]),
                TaskRequestType::Private => {
                    if is_current_user_donor {
                        self.get_follower_ids(current_user_id).await
                    } else {
                        Ok(vec![])
                    }
                }
            },
        }
    }

    async fn get_follower_ids(&self, user: RecordId) -> CtxResult<Vec<RecordId>> {
        Ok(self.follow_repository.user_follower_ids(user).await?)
    }
}
