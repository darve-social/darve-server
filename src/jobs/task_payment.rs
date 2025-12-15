use std::{sync::Arc, time::Duration};

use crate::{
    middleware::{ctx::Ctx, mw_ctx::CtxState},
    services::{notification_service::NotificationService, task_service::TaskService},
};

use tokio::task::JoinHandle;

pub async fn run(state: Arc<CtxState>, delay: Duration) -> JoinHandle<()> {
    let state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(delay).await;

            let ctx = Ctx::new(Ok("".to_string()), false);
            let task_service = TaskService::new(
                &state.db.client,
                &ctx,
                &state.db.task_request,
                &state.db.task_donors,
                &state.db.task_participants,
                &state.db.access,
                &state.db.tags,
                NotificationService::new(
                    &state.db.client,
                    &ctx,
                    &state.event_sender,
                    &state.db.user_notifications,
                ),
            );

            if let Err(err) = task_service.distribute_expired_tasks_rewards().await {
                println!("Error distributing rewards: {:?}", err);
            }
        }
    })
}
