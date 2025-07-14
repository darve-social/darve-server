use std::{sync::Arc, time::Duration};

use crate::{
    middleware::{ctx::Ctx, mw_ctx::CtxState},
    services::task_service::TaskService,
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
                &&state.event_sender,
                &state.db.user_notifications,
                &state.db.task_donors,
                &state.db.task_request_users,
            );

            if let Err(err) = task_service.distribute_expired_tasks_rewards().await {
                println!("Error distributing rewards: {:?}", err);
            }
        }
    })
}
