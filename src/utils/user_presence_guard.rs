use std::sync::Arc;

use crate::{
    entities::user_auth::local_user_entity::LocalUserDbService,
    middleware::{
        ctx::Ctx,
        mw_ctx::{AppEvent, AppEventType, AppEventUsetStatus, CtxState},
    },
};

pub struct UserPresenceGuard {
    state: Arc<CtxState>,
    user_id: String,
}

impl UserPresenceGuard {
    pub fn new(state: Arc<CtxState>, user_id: String) -> Self {
        let online_users = state.online_users.clone();
        let mut count = online_users.entry(user_id.clone()).or_insert(0);
        *count += 1;

        if *count == 1 {
            let _ = state.event_sender.send(AppEvent {
                user_id: user_id.clone(),
                metadata: None,
                content: None,
                receivers: vec![],
                event: AppEventType::UserStatus(AppEventUsetStatus { is_online: true }),
            });
        }

        println!(
            "UserPresenceGuard start: user_id: {:?}, count: {:?}",
            user_id, count
        );
        Self { state, user_id }
    }
}

impl Drop for UserPresenceGuard {
    fn drop(&mut self) {
        let user_id = self.user_id.clone();
        let online_users = self.state.online_users.clone();
        let event = self.state.event_sender.clone();
        let db = self.state.db.client.clone();
        println!("UserPresenceGuard drop: user_id: {:?}", user_id);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            if let Some(mut entry) = online_users.get_mut(&user_id) {
                *entry -= 1;
                if *entry > 0 {
                    return;
                }
            }
            let _ = event.send(AppEvent {
                user_id: user_id.clone(),
                metadata: None,
                content: None,
                receivers: vec![],
                event: AppEventType::UserStatus(AppEventUsetStatus { is_online: false }),
            });

            online_users.remove(&user_id);
            let user_db_service = LocalUserDbService {
                db: &db,
                ctx: &Ctx::new(Ok(user_id.clone()), false),
            };
            if let Err(err) = user_db_service.update_last_seen(&user_id).await {
                eprintln!("Failed to save last_seen for {}: {:?}", user_id, err);
            }
            println!("UserPresenceGuard droped: user_id: {:?}", user_id);
        });
    }
}
