use crate::database::client::Db;
use crate::entities::task_request_user::TaskParticipantStatus;
use crate::entities::user_auth::local_user_entity;
use crate::entities::wallet::wallet_entity::{self};
use crate::entities::wallet::wallet_entity::{Wallet, TABLE_NAME as WALLET_TABLE_NAME};
use crate::middleware;
use crate::middleware::utils::db_utils::get_entity_view;
use chrono::{DateTime, TimeDelta, Utc};
use middleware::utils::db_utils::{
    get_entity, get_entity_list_view, with_not_found_err, IdentIdName, Pagination,
    ViewFieldSelector,
};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use surrealdb::sql::{Datetime, Thing};
use wallet_entity::CurrencySymbol;

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub created_by: Thing,
    pub request_txt: String,
    pub deliverable_type: DeliverableType,
    pub r#type: TaskRequestType,
    pub reward_type: RewardType,
    pub currency: CurrencySymbol,
    pub created_at: DateTime<Utc>,
    pub acceptance_period: u16,
    pub delivery_period: u16,
    pub wallet_id: Thing,
    pub status: TaskRequestStatus,
    pub due_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum TaskRequestStatus {
    InProgress,
    Completed,
}

#[derive(Debug, Serialize)]
pub struct TaskRequestCreate {
    pub from_user: Thing,
    pub request_txt: String,
    pub deliverable_type: DeliverableType,
    pub r#type: TaskRequestType,
    pub reward_type: RewardType,
    pub currency: CurrencySymbol,
    pub acceptance_period: u16,
    pub delivery_period: u16,
}

#[derive(Debug, Deserialize)]
pub struct TaskDonorForReward {
    pub amount: i64,
    pub id: Thing,
}

#[derive(Debug, Deserialize)]
pub struct TaskParticipantForReward {
    pub id: Thing,
    pub user_id: Thing,
    pub status: TaskParticipantStatus,
    pub reward_tx: Option<Thing>,
}

#[derive(Debug, Deserialize)]
pub struct TaskForReward {
    pub id: Thing,
    pub currency: CurrencySymbol,
    pub donors: Vec<TaskDonorForReward>,
    pub participants: Vec<TaskParticipantForReward>,
    pub wallet: Wallet,
    pub balance: i64,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub enum TaskRequestType {
    Open,
    Close,
}

#[derive(Display, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RewardType {
    // needs to be same as col name
    // #[strum(to_string = "on_delivery")]
    OnDelivery, // paid when delivered
                // #[strum(to_string = "vote_winner")]
                // VoteWinner { voting_period_min: i64 }, // paid after voting
}

#[derive(EnumString, Display, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DeliverableType {
    PublicPost,
    // Participants,
}

pub struct TaskRequestDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

impl<'a> TaskRequestDbService<'a> {}

pub const TABLE_NAME: &str = "task_request";
const TABLE_COL_USER: &str = local_user_entity::TABLE_NAME;

impl<'a> TaskRequestDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let curr_usd = CurrencySymbol::USD.to_string();
        let curr_reef = CurrencySymbol::REEF.to_string();
        let curr_eth = CurrencySymbol::ETH.to_string();

        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS created_by ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE INDEX IF NOT EXISTS created_by_user_idx ON TABLE {TABLE_NAME} COLUMNS created_by;
    DEFINE FIELD IF NOT EXISTS deliverable_type ON TABLE {TABLE_NAME} TYPE {{ type: \"PublicPost\"}};
    DEFINE FIELD IF NOT EXISTS request_txt ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD IF NOT EXISTS reward_type ON TABLE {TABLE_NAME} TYPE {{ type: \"OnDelivery\"}} | {{ type: \"VoteWinner\", voting_period_min: int }};
    DEFINE FIELD IF NOT EXISTS currency ON TABLE {TABLE_NAME} TYPE '{curr_usd}'|'{curr_reef}'|'{curr_eth}';
    DEFINE FIELD IF NOT EXISTS type ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD IF NOT EXISTS status ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD IF NOT EXISTS due_at ON TABLE {TABLE_NAME} TYPE datetime;
    DEFINE FIELD IF NOT EXISTS acceptance_period ON TABLE {TABLE_NAME} TYPE number;
    DEFINE FIELD IF NOT EXISTS delivery_period ON TABLE {TABLE_NAME} TYPE number;
    DEFINE FIELD IF NOT EXISTS wallet_id ON TABLE {TABLE_NAME} TYPE record<{WALLET_TABLE_NAME}>;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now()  VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE time::now();
    DEFINE INDEX IF NOT EXISTS idx_status ON TABLE {TABLE_NAME} COLUMNS status;
    DEFINE INDEX IF NOT EXISTS idx_due_at ON TABLE {TABLE_NAME} COLUMNS due_at;
    ");
        let mutation = self.db.query(sql).await?;

        mutation.check().expect("should mutate taskRequest");

        Ok(())
    }

    pub async fn create(&self, record: TaskRequestCreate) -> CtxResult<TaskRequest> {
        let hours = (record.delivery_period + record.acceptance_period) as i64;
        let due_at = Utc::now().checked_add_signed(TimeDelta::hours(hours));
        let mut res = self
            .db
            .query("BEGIN TRANSACTION")
            .query("LET $wallet_id=(CREATE wallet SET transaction_head = {{}} RETURN id)[0].id;")
            .query(format!(
                "CREATE {TABLE_NAME} SET
                delivery_period=$delivery_period,
                wallet_id=$wallet_id,
                created_by=$created_by,
                deliverable_type=$deliverable_type,
                request_txt=$request_txt,
                reward_type=$reward_type,
                currency=$currency,
                type=$type,
                acceptance_period=$acceptance_period,
                due_at=$due_at,
                status=$status;"
            ))
            .query("COMMIT TRANSACTION")
            .bind(("delivery_period", record.delivery_period))
            .bind(("created_by", record.from_user.clone()))
            .bind(("deliverable_type", record.deliverable_type.clone()))
            .bind(("request_txt", record.request_txt.clone()))
            .bind(("reward_type", record.reward_type.clone()))
            .bind(("currency", record.currency.clone()))
            .bind(("type", record.r#type.clone()))
            .bind(("acceptance_period", record.acceptance_period))
            .bind(("status", TaskRequestStatus::InProgress))
            .bind(("due_at", Datetime::from(due_at.unwrap())))
            .await
            .map_err(CtxError::from(self.ctx))?;
        let data = res.take::<Option<TaskRequest>>(res.num_statements() - 1)?;
        Ok(data.unwrap())
    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<TaskRequest> {
        let opt = get_entity::<TaskRequest>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }

    pub async fn get_by_id<T: for<'de> Deserialize<'de> + ViewFieldSelector>(
        &self,
        id: &Thing,
    ) -> CtxResult<T> {
        let opt = get_entity_view::<T>(
            &self.db,
            TABLE_NAME.to_string(),
            &IdentIdName::Id(id.clone()),
        )
        .await?;
        with_not_found_err(opt, self.ctx, &id.to_raw())
    }

    pub async fn get_by_user<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        user: &Thing,
        status: Option<TaskParticipantStatus>,
    ) -> CtxResult<Vec<T>> {
        let status_query = match &status {
            Some(_) => "AND $status IN ->task_participant.status",
            None => "",
        };
        let query = format!(
            "SELECT {} FROM {TABLE_NAME} WHERE $user IN ->task_participant.out {};",
            &T::get_select_query_fields(),
            status_query
        );
        let mut res = self
            .db
            .query(query)
            .bind(("user", user.clone()))
            .bind(("status", status))
            .await?;
        Ok(res.take::<Vec<T>>(0)?)
    }

    pub async fn get_by_creator<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        user: Thing,
        pagination: Option<Pagination>,
    ) -> CtxResult<Vec<T>> {
        let filter_by = vec![IdentIdName::ColumnIdent {
            column: "created_by".to_string(),
            val: user.to_raw(),
            rec: true,
        }];
        get_entity_list_view::<T>(
            self.db,
            TABLE_NAME.to_string(),
            &IdentIdName::ColumnIdentAnd(filter_by),
            pagination,
        )
        .await
    }

    pub async fn on_post_list_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        post_id: Thing,
    ) -> CtxResult<Vec<T>> {
        get_entity_list_view::<T>(
            self.db,
            TABLE_NAME.to_string(),
            &IdentIdName::ColumnIdent {
                column: "on_post".to_string(),
                val: post_id.to_raw(),
                rec: true,
            },
            None,
        )
        .await
    }

    pub async fn update_status(&self, task: Thing, status: TaskRequestStatus) -> CtxResult<()> {
        self.db
            .query("UPDATE $id SET status=$status;")
            .bind(("id", task))
            .bind(("status", status))
            .await?
            .check()?;
        Ok(())
    }

    pub async fn get_ready_for_payment(&self) -> CtxResult<Vec<TaskForReward>> {
        let query = format!(
            "SELECT *, wallet.transaction_head[currency].balance as balance
             FROM (
                SELECT id, wallet_id.* AS wallet, currency,
                    ->task_participant.{{ status, id, user_id: out }} AS participants,
                    ->task_donor.{{ id: out, amount: transaction.amount_out }} AS donors
                FROM {TABLE_NAME}
                WHERE status != $status AND due_at <= time::now()
            )"
        );
        let mut res = self
            .db
            .query(query)
            .bind(("status", TaskRequestStatus::Completed))
            .await?;
        let data = res.take::<Vec<TaskForReward>>(0)?;
        Ok(data)
    }
}
