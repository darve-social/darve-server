use crate::{
    entities::{
        task_request_user::{TaskParticipantStatus, TaskParticipantTimeline},
        wallet::wallet_entity::CurrencySymbol,
    },
    middleware::utils::db_utils::{ViewFieldSelector, ViewRelateField},
    models::view::user::UserView,
};
use ::serde::{Deserialize, Serialize};
use chrono::{DateTime, TimeDelta, Utc};
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
    pub participants: Vec<TaskRequestViewParticipant>,
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

#[derive(Deserialize, Serialize, Debug)]
pub struct TaskViewForParticipant {
    pub id: Thing,
    pub created_by: UserView,
    pub request_txt: String,
    pub total_amount: u64,
    pub donors: Vec<TaskRequestDonorView>,
    pub currency: CurrencySymbol,
    pub end_at: DateTime<Utc>,
    pub accept_by: DateTime<Utc>,
    pub deliver_by: Option<DateTime<Utc>>,
    pub participants: Vec<TaskRequestViewParticipant>,
    pub created_at: DateTime<Utc>,
}

impl TaskViewForParticipant {
    pub fn from_view(view: TaskRequestView, participant_id: &Thing) -> Self {
        let amount = view.donors.iter().fold(0, |res, d| res + d.amount) as u64;

        let participant = view
            .participants
            .into_iter()
            .find(|p| &p.user.id == participant_id)
            .expect("Participant not found");

        let deliver_by = if participant.status == TaskParticipantStatus::Accepted {
            let timelines = participant.timelines.as_ref();
            let date = timelines.unwrap().last().unwrap().date.clone();
            date.checked_add_signed(TimeDelta::hours(view.delivery_period as i64))
        } else {
            None
        };

        Self {
            id: view.id,
            created_by: view.created_by,
            request_txt: view.request_txt,
            total_amount: amount,
            donors: view.donors,
            currency: view.currency,
            end_at: view.due_at,
            participants: vec![participant],
            accept_by: view
                .created_at
                .checked_add_signed(TimeDelta::hours(view.acceptance_period as i64))
                .unwrap(),

            deliver_by,
            created_at: view.created_at,
        }
    }
}
