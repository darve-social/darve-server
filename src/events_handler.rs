use community_entity::CommunityDbService;
use middleware::mw_ctx::{ApplicationEvent, CtxState};
use middleware::utils::db_utils::RecordWithId;
use middleware::utils::extractor_utils::DiscussionParams;
use middleware::utils::string_utils::get_string_thing;
use surrealdb::sql::Thing;
use tokio::task::JoinHandle;

use crate::entities::community::community_entity;
use crate::entities::community::post_entity::PostDbService;
use crate::entities::community::post_stream_entity::PostStreamDbService;
use crate::{entities, middleware};

pub fn application_event_handler(ctx_state: &CtxState) -> JoinHandle<()> {
    let tx = ctx_state.application_event.clone();
    let mut rx = tx.subscribe();
    let db = ctx_state._db.clone();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                ApplicationEvent::UserFollowAdded {
                    ctx,
                    follow_user_id,
                } => {
                    let evt_name = ApplicationEvent::UserFollowAdded {
                        ctx: ctx.clone(),
                        follow_user_id: follow_user_id.clone(),
                    }
                    .to_string();
                    let Ok(user_id) = ctx.user_id() else {
                        println!("ERROR events_handler.rs {evt_name}: No ctx.user_id()");
                        return;
                    };
                    let Ok(user_thing) = Thing::try_from(user_id) else {
                        println!("ERROR events_handler.rs {evt_name}: user_id not Thing");
                        return;
                    };
                    let post_db_service = PostDbService { ctx: &ctx, db: &db };
                    let Ok(following_thing) = get_string_thing(follow_user_id) else {
                        println!("ERROR events_handler.rs {evt_name}: follow_user_id not Thing");
                        return;
                    };

                    // TODO -profile-discussion- get profile discussion from user id like [discussion_table]:[user_id_id] so no query is required
                    let follow_profile_comm = match (CommunityDbService { ctx: &ctx, db: &db }
                        .get_profile_community(following_thing)
                        .await)
                    {
                        Ok(res) => res,
                        Err(err) => {
                            println!("ERROR events_handler.rs {evt_name}: get_profile_community error / err={err:?}");
                            return;
                        }
                    };
                    let Some(follow_profile_discussion_id) = follow_profile_comm.profile_discussion
                    else {
                        println!("ERROR events_handler.rs {evt_name}: No value for follow_profile_comm.profile_discussion");
                        return;
                    };

                    let latest_posts = match post_db_service
                        .get_by_discussion_desc_view::<RecordWithId>(
                            follow_profile_discussion_id,
                            DiscussionParams {
                                topic_id: None,
                                start: Some(0),
                                count: Some(3),
                            },
                        )
                        .await
                    {
                        Ok(res) => res,
                        Err(err) => {
                            println!("ERROR events_handler.rs {evt_name}: err getting latest posts / err={err:?}");
                            return;
                        }
                    };

                    let stream_db_service = PostStreamDbService { ctx: &ctx, db: &db };
                    for post in latest_posts {
                        if let Err(err) = stream_db_service
                            .add_to_users_stream(vec![user_thing.clone()], &post.id)
                            .await
                        {
                            println!("ERROR events_handler.rs {evt_name}: error adding to stream / err{err:?}");
                            continue;
                        };
                    }
                }
            }
        }
    })
}
