use sb_community::entity::{post_entitiy::PostDbService, post_stream_entitiy::PostStreamDbService};
use sb_middleware::mw_ctx::{ApplicationEvent, CtxState};
use surrealdb::sql::Thing;
use tokio::task::JoinHandle;

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
                    let user_id = ctx.user_id().unwrap();
                    let user_thing = Thing::try_from(user_id).unwrap();
                    let post_db_service = PostDbService { ctx: &ctx, db: &db };
                    let latest_posts = post_db_service
                        .get_latest(&Thing::try_from(follow_user_id).unwrap(), 3)
                        .await
                        .unwrap_or_default();

                    if latest_posts.is_empty() {
                        return;
                    }

                    let stream_db_service = PostStreamDbService { ctx: &ctx, db: &db };
                    for post in latest_posts {
                        let _ = stream_db_service
                            .add_to_users_stream(vec![user_thing.clone()], &post.id.unwrap())
                            .await;
                    }
                }
            }
        }
    })
}
