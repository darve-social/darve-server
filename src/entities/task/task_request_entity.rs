use crate::database::client::Db;
use crate::entities::community::post_entity;
use crate::entities::task_request_user::TaskRequestUserStatus;
use crate::entities::user_auth::local_user_entity;
use crate::entities::wallet::wallet_entity;
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
use surrealdb::sql::Thing;
use wallet_entity::CurrencySymbol;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub from_user: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_post: Option<Thing>,
    pub request_txt: String,
    pub deliverable_type: DeliverableType,
    pub r#type: TaskRequestType,
    pub reward_type: RewardType,
    pub currency: CurrencySymbol,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
    pub acceptance_period: Option<u16>,
    pub delivery_period: u16,
}

impl TaskRequest {
    pub fn can_still_use(start: DateTime<Utc>, period: Option<u16>) -> bool {
        match period {
            Some(value) => {
                start
                    .checked_add_signed(TimeDelta::hours(value.into()))
                    .unwrap()
                    > Utc::now()
            }
            _ => true,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct TaskRequestCreate {
    pub id: Thing,
    pub from_user: Thing,
    pub on_post: Option<Thing>,
    pub request_txt: String,
    pub deliverable_type: DeliverableType,
    pub r#type: TaskRequestType,
    pub reward_type: RewardType,
    pub currency: CurrencySymbol,
    pub acceptance_period: Option<u16>,
    pub delivery_period: u16,
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
const TABLE_COL_POST: &str = post_entity::TABLE_NAME;
const TABLE_COL_USER: &str = local_user_entity::TABLE_NAME;

impl<'a> TaskRequestDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let curr_usd = CurrencySymbol::USD.to_string();
        let curr_reef = CurrencySymbol::REEF.to_string();
        let curr_eth = CurrencySymbol::ETH.to_string();

        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS on_post ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_COL_POST}>>;
    DEFINE INDEX IF NOT EXISTS on_post_idx ON TABLE {TABLE_NAME} COLUMNS on_post;
    DEFINE FIELD IF NOT EXISTS from_user ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE INDEX IF NOT EXISTS from_user_idx ON TABLE {TABLE_NAME} COLUMNS from_user;
    DEFINE FIELD IF NOT EXISTS deliverable_type ON TABLE {TABLE_NAME} TYPE {{ type: \"PublicPost\"}};
    DEFINE FIELD IF NOT EXISTS request_txt ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD IF NOT EXISTS reward_type ON TABLE {TABLE_NAME} TYPE {{ type: \"OnDelivery\"}} | {{ type: \"VoteWinner\", voting_period_min: int }};
    DEFINE FIELD IF NOT EXISTS currency ON TABLE {TABLE_NAME} TYPE '{curr_usd}'|'{curr_reef}'|'{curr_eth}';
    DEFINE FIELD IF NOT EXISTS type ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD IF NOT EXISTS acceptance_period ON TABLE {TABLE_NAME} TYPE option<number>;
    DEFINE FIELD IF NOT EXISTS delivery_period ON TABLE {TABLE_NAME} TYPE number;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now()  VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.db.query(sql).await?;

        mutation.check().expect("should mutate taskRequest");

        Ok(())
    }

    pub async fn create(&self, record: TaskRequestCreate) -> CtxResult<TaskRequest> {
        let res = self
            .db
            .create(TABLE_NAME)
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<TaskRequest>| v.unwrap());

        // let things: Vec<Domain> = self.db.select(TABLE_NAME).await.ok().unwrap();
        // dbg!(things);
        res
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
        status: Option<TaskRequestUserStatus>,
    ) -> CtxResult<Vec<T>> {
        let status_query = match &status {
            Some(_) => "AND $status IN ->task_request_user.status",
            None => "",
        };
        let query = format!(
            "SELECT {} FROM {TABLE_NAME} WHERE $user IN ->task_request_user.out {};",
            &T::get_select_query_fields(&IdentIdName::Id(user.clone())),
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
        post_id: Option<Thing>,
        pagination: Option<Pagination>,
    ) -> CtxResult<Vec<T>> {
        let mut filter_by = vec![IdentIdName::ColumnIdent {
            column: "from_user".to_string(),
            val: user.to_raw(),
            rec: true,
        }];
        if post_id.is_some() {
            filter_by.push(IdentIdName::ColumnIdent {
                column: "on_post".to_string(),
                val: post_id.expect("checked is some").to_raw(),
                rec: true,
            })
        }
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
}
