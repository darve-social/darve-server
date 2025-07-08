use std::{sync::Arc, time::Duration};

use serde::Deserialize;
use tokio::task::JoinHandle;

use crate::{
    entities::{
        task::task_request_participation_entity::TaskRequestParticipation,
        task_request_user::{TaskRequestUser, TaskRequestUserStatus},
    },
    middleware::mw_ctx::CtxState,
};

#[derive(Debug, Deserialize)]
pub struct TaskReadyForPayment {
    pub participants: Vec<TaskRequestParticipation>,
    pub users: Vec<TaskRequestUser>,
}

pub async fn run(state: Arc<CtxState>) -> JoinHandle<()> {
    let state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;

            let query = "
            SELECT id,
                ->task_request_user.{ status, id: out } as users,
                ->task_request_participation.{id:out, lock, amount, currency} as participants
            FROM task_request
            WHERE delivery_period != None AND
                created_at + <duration>string::concat(delivery_period, 'h') <= time::now() AND
                array::any(->task_request_participant.out.lock) != None AND 
                $status IN ->task_request_user.*.status";

            let res = state
                .db
                .client
                .query(query)
                .bind(("status", TaskRequestUserStatus::Delivered))
                .await;

            if res.is_err() {
                continue;
            }

            let data = res
                .unwrap()
                .take::<Vec<TaskReadyForPayment>>(0)
                .unwrap_or(vec![]);

            data.iter().for_each(|task| {
                let _delivered_users = task
                    .users
                    .iter()
                    .filter(|u| u.status == TaskRequestUserStatus::Delivered)
                    .collect::<Vec<&TaskRequestUser>>();

                let _amount = task
                    .participants
                    .iter()
                    .fold(0, |res, item| res + item.amount);

                // payment process
            });
        }
    })
}
