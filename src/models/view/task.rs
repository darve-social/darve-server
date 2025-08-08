use crate::{
    entities::{
        task_request_user::{TaskParticipantStatus, TaskParticipantTimeline},
        wallet::wallet_entity::CurrencySymbol,
    },
    middleware::utils::db_utils::{ViewFieldSelector, ViewRelateField},
    models::view::user::UserView,
};
use ::serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use surrealdb::sql::Thing;

#[derive(Deserialize, Serialize, Debug)]
pub struct TaskRequestViewParticipant {
    pub user: UserView,
    pub status: TaskParticipantStatus,
    pub timelines: Option<Vec<TaskParticipantTimeline>>,
}
#[derive(Deserialize, Serialize, Debug)]
pub struct TaskRequestDonorView {
    pub id: Thing,
    pub user: UserView,
    pub amount: i64,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct TaskRequestView {
    pub id: Thing,
    pub created_by: UserView,
    pub participants: Option<Vec<TaskRequestViewParticipant>>,
    pub request_txt: String,
    pub donors: Vec<TaskRequestDonorView>,
    pub currency: CurrencySymbol,
    pub acceptance_period: u16,
    pub delivery_period: u16,
    pub due_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub wallet_id: Thing,
}

impl ViewFieldSelector for TaskRequestView {
    fn get_select_query_fields() -> String {
        "id,
        due_at,
        created_at,
        delivery_period,
        acceptance_period,
        wallet_id,
        currency,
        request_txt,
        created_by.* as created_by,
        ->task_participant.{ user: out.*, status, timelines } as participants,
        ->task_donor.{id, user: out.*, amount: transaction.amount_out} as donors"
            .to_string()
    }
}

impl ViewRelateField for TaskRequestView {
    fn get_fields() -> &'static str {
        "id,
        due_at,
        created_at,
        delivery_period,
        acceptance_period,
        wallet_id,
        currency,
        request_txt,
        created_by:created_by.*,
        participants:->task_participant.{ user: out.*, status, timelines},
        donors:->task_donor.{id, user: out.*, amount: transaction.amount_out}"
    }
}
