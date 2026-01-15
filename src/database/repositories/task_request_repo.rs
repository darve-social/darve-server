use crate::database::repository_impl::Repository;
use crate::database::surrdb_utils::get_thing;
use crate::database::table_names::ACCESS_TABLE_NAME;
use crate::database::table_names::TASK_REQUEST_TABLE_NAME;
use crate::entities::community::discussion_entity::DiscussionType;
use crate::entities::community::discussion_entity::TABLE_NAME as DISC_TABLE_NAME;
use crate::entities::community::post_entity::PostType;
use crate::entities::community::post_entity::TABLE_NAME as POST_TABLE_NAME;
use crate::entities::task_request::{
    TaskForReward, TaskRequestCreate, TaskRequestEntity, TaskRequestStatus, TaskRequestType,
};
use crate::entities::task_request_user::TaskParticipantStatus;
use crate::entities::user_auth::local_user_entity;
use crate::entities::wallet::wallet_entity::TABLE_NAME as WALLET_TABLE_NAME;
use crate::entities::wallet::wallet_entity::{CurrencySymbol, TRANSACTION_HEAD_F};
use crate::interfaces::repositories::task_request_ifce::TaskRequestRepositoryInterface;
use crate::middleware::error::AppError;
use crate::middleware::error::AppResult;
use crate::middleware::utils::db_utils::{Pagination, QryOrder, ViewFieldSelector};
use async_trait::async_trait;
use chrono::{TimeDelta, Utc};
use serde::Deserialize;
use surrealdb::method::Query;
use surrealdb::sql::{Datetime, Thing};

const TABLE_COL_USER: &str = local_user_entity::TABLE_NAME;

impl Repository<TaskRequestEntity> {
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let curr_usd = CurrencySymbol::USD.to_string();
        let curr_reef = CurrencySymbol::REEF.to_string();
        let curr_eth = CurrencySymbol::ETH.to_string();

        // ⚠️ CRITICAL: Use single quotes for object type string literals!
        // ⚠️ CRITICAL: Keep field name "created_at" - same as old entity!
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TASK_REQUEST_TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS belongs_to ON TABLE {TASK_REQUEST_TABLE_NAME} TYPE record<{DISC_TABLE_NAME}|{POST_TABLE_NAME}>;
    DEFINE FIELD IF NOT EXISTS created_by ON TABLE {TASK_REQUEST_TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE FIELD IF NOT EXISTS deliverable_type ON TABLE {TASK_REQUEST_TABLE_NAME} TYPE {{ type: 'PublicPost'}};
    DEFINE FIELD IF NOT EXISTS request_txt ON TABLE {TASK_REQUEST_TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD IF NOT EXISTS reward_type ON TABLE {TASK_REQUEST_TABLE_NAME} TYPE {{ type: 'OnDelivery'}} | {{ type: 'VoteWinner', voting_period_min: int }};
    DEFINE FIELD IF NOT EXISTS currency ON TABLE {TASK_REQUEST_TABLE_NAME} TYPE '{curr_usd}'|'{curr_reef}'|'{curr_eth}';
    DEFINE FIELD IF NOT EXISTS type ON TABLE {TASK_REQUEST_TABLE_NAME} TYPE string;
    DEFINE FIELD IF NOT EXISTS status ON TABLE {TASK_REQUEST_TABLE_NAME} TYPE string;
    DEFINE FIELD IF NOT EXISTS due_at ON TABLE {TASK_REQUEST_TABLE_NAME} TYPE datetime;
    DEFINE FIELD IF NOT EXISTS acceptance_period ON TABLE {TASK_REQUEST_TABLE_NAME} TYPE number;
    DEFINE FIELD IF NOT EXISTS delivery_period ON TABLE {TASK_REQUEST_TABLE_NAME} TYPE number;
    DEFINE FIELD IF NOT EXISTS wallet_id ON TABLE {TASK_REQUEST_TABLE_NAME} TYPE record<{WALLET_TABLE_NAME}>;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE {TASK_REQUEST_TABLE_NAME} TYPE datetime DEFAULT time::now()  VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {TASK_REQUEST_TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE time::now();
    DEFINE INDEX IF NOT EXISTS idx_status ON TABLE {TASK_REQUEST_TABLE_NAME} COLUMNS status;
    DEFINE INDEX IF NOT EXISTS idx_due_at ON TABLE {TASK_REQUEST_TABLE_NAME} COLUMNS due_at;
    DEFINE INDEX IF NOT EXISTS belongs_to_idx ON TABLE {TASK_REQUEST_TABLE_NAME} COLUMNS belongs_to;
    DEFINE INDEX IF NOT EXISTS created_by_user_idx ON TABLE {TASK_REQUEST_TABLE_NAME} COLUMNS created_by;
    ");
        let mutation = self.client.query(sql).await?;
        mutation.check().expect("should mutate taskRequest");
        Ok(())
    }
}

#[async_trait]
impl TaskRequestRepositoryInterface for Repository<TaskRequestEntity> {
    fn build_create_query<'b>(
        &self,
        query: Query<'b, surrealdb::engine::any::Any>,
        record: &TaskRequestCreate,
    ) -> Query<'b, surrealdb::engine::any::Any> {
        let seconds = (record.delivery_period + record.acceptance_period) as i64;
        let due_at = Utc::now().checked_add_signed(TimeDelta::seconds(seconds));
        let mut gry = query
            .query(format!(
                "CREATE ONLY $_task_wallet_id SET {TRANSACTION_HEAD_F} = {{{{}}}};"
            ))
            .query(format!(
                "LET $task = CREATE $_task_id SET
                    delivery_period=$_task_delivery_period,
                    belongs_to=$_task_belongs_to,
                    wallet_id=$_task_wallet_id,
                    created_by=$_task_created_by,
                    deliverable_type=$_task_deliverable_type,
                    request_txt=$_task_request_txt,
                    reward_type=$_task_reward_type,
                    currency=$_task_currency,
                    type=$_task_type,
                    acceptance_period=$_task_acceptance_period,
                    due_at=$_task_due_at,
                    status=$_task_status;"
            ));

        if record.increase_tasks_nr_for_belongs {
            gry = gry.query("UPDATE $_task_belongs_to SET tasks_nr+=1;")
        }

        gry = gry
            .bind(("_task_delivery_period", record.delivery_period))
            .bind(("_task_belongs_to", record.belongs_to.clone()))
            .bind(("_task_created_by", record.from_user.clone()))
            .bind(("_task_deliverable_type", record.deliverable_type.clone()))
            .bind(("_task_request_txt", record.request_txt.clone()))
            .bind(("_task_reward_type", record.reward_type.clone()))
            .bind(("_task_currency", record.currency.clone()))
            .bind(("_task_type", record.r#type.clone()))
            .bind(("_task_acceptance_period", record.acceptance_period))
            .bind(("_task_status", TaskRequestStatus::Init))
            .bind(("_task_due_at", Datetime::from(due_at.unwrap())))
            .bind(("_task_wallet_id", record.wallet_id.clone()))
            .bind(("_task_id", record.task_id.clone()));

        gry
    }

    async fn get_by_posts<T: for<'de> Deserialize<'de> + ViewFieldSelector + Send>(
        &self,
        posts: Vec<Thing>,
        user: Thing,
    ) -> Result<Vec<T>, surrealdb::Error> {
        let fields = T::get_select_query_fields();
        let query = format!("
            SELECT {fields} FROM {TASK_REQUEST_TABLE_NAME}
            WHERE belongs_to IN $posts
                AND (belongs_to.type IN $public_post_types OR $user IN belongs_to<-{ACCESS_TABLE_NAME}.in)
                AND (belongs_to.belongs_to.type=$disc_type OR $user IN belongs_to.belongs_to<-{ACCESS_TABLE_NAME}.in)");
        let mut res = self
            .client
            .query(query)
            .bind(("posts", posts))
            .bind(("user", user))
            .bind(("disc_type", DiscussionType::Public))
            .bind(("public_post_types", [PostType::Public, PostType::Idea]))
            .await?;
        Ok(res.take::<Vec<T>>(0)?)
    }

    async fn get_by_public_disc<T: for<'de> Deserialize<'de> + ViewFieldSelector + Send>(
        &self,
        disc_id: &str,
        user_id: &str,
        filter_by_type: Option<TaskRequestType>,
        pag: Option<Pagination>,
        is_ended: Option<bool>,
        is_acceptance_expired: Option<bool>,
    ) -> Result<Vec<T>, surrealdb::Error> {
        let acceptance_condition = match is_acceptance_expired {
            Some(value) => match value {
                true => "AND (created_at + type::duration(string::concat(acceptance_period, 's'))) <= time::now()",
                false => "AND (created_at + type::duration(string::concat(acceptance_period, 's'))) > time::now()",
            },
            None => "",
        };
        let date_condition = match is_ended {
            Some(value) => match value {
                true => "AND due_at <= time::now()",
                false => "AND due_at > time::now()",
            },
            None => "",
        };
        let type_condition = match &filter_by_type {
            Some(_) => "AND type=$filter_by_type",
            None => "",
        };

        let pagination_str = match pag {
            Some(ref p) => format!(
                "ORDER BY created_at {} LIMIT {} START {}",
                p.order_dir.as_ref().unwrap_or(&QryOrder::DESC),
                p.count,
                p.start
            ),
            None => "".into(),
        };

        let fields = T::get_select_query_fields();
        let query = format!("SELECT {fields}, created_at FROM {TASK_REQUEST_TABLE_NAME}
                WHERE belongs_to=$disc {type_condition} {date_condition} {acceptance_condition} AND (type=$task_type OR $user IN <-{ACCESS_TABLE_NAME}.in)
                {pagination_str};");

        let mut res = self
            .client
            .query(query)
            .bind(("user", Thing::from((TABLE_COL_USER, user_id))))
            .bind(("disc", Thing::from((DISC_TABLE_NAME, disc_id))))
            .bind(("task_type", TaskRequestType::Public))
            .bind(("filter_by_type", filter_by_type))
            .await?;

        Ok(res.take::<Vec<T>>(0)?)
    }

    async fn get_by_private_disc<T: for<'de> Deserialize<'de> + ViewFieldSelector + Send>(
        &self,
        disc_id: &str,
        user_id: &str,
        filter_by_type: Option<TaskRequestType>,
        pag: Option<Pagination>,
        is_ended: Option<bool>,
        is_acceptance_expired: Option<bool>,
    ) -> Result<Vec<T>, surrealdb::Error> {
        let acceptance_condition = match is_acceptance_expired {
            Some(value) => match value {
                true => "AND (created_at + type::duration(string::concat(acceptance_period, 's'))) <= time::now()",
                false => "AND (created_at + type::duration(string::concat(acceptance_period, 's'))) > time::now()",
            },
            None => "",
        };
        let date_condition = match is_ended {
            Some(value) => match value {
                true => "AND due_at <= time::now()",
                false => "AND due_at > time::now()",
            },
            None => "",
        };
        let type_condition = match &filter_by_type {
            Some(_) => "AND type=$filter_by_type",
            None => "",
        };

        let pagination_str = match pag {
            Some(ref p) => format!(
                "ORDER BY created_at {} LIMIT {} START {}",
                p.order_dir.as_ref().unwrap_or(&QryOrder::DESC),
                p.count,
                p.start
            ),
            None => "".into(),
        };

        let fields = T::get_select_query_fields();
        let query = format!(
            "SELECT {fields}, created_at FROM {TASK_REQUEST_TABLE_NAME}
            WHERE belongs_to=$disc {type_condition} {date_condition} {acceptance_condition} AND $user IN belongs_to<-{ACCESS_TABLE_NAME}.in
            {pagination_str};"
        );
        let mut res = self
            .client
            .query(query)
            .bind(("user", Thing::from((TABLE_COL_USER, user_id))))
            .bind(("disc", Thing::from((DISC_TABLE_NAME, disc_id))))
            .bind(("filter_by_type", filter_by_type))
            .await?;

        Ok(res.take::<Vec<T>>(0)?)
    }

    async fn get_by_user_and_disc<T: for<'de> Deserialize<'de> + ViewFieldSelector + Send>(
        &self,
        user_id: &str,
        disc_id: &str,
        status: Option<TaskParticipantStatus>,
    ) -> Result<Vec<T>, surrealdb::Error> {
        let status_condition = match &status {
            Some(_) => "AND status=$status",
            None => "",
        };

        let fields = T::get_select_query_fields();
        let query = format!(
            "SELECT {fields} FROM {TASK_REQUEST_TABLE_NAME} WHERE ->task_participant[WHERE out=$user {status_condition}]"
        );

        let mut res = self
            .client
            .query(query)
            .bind(("user", Thing::from((TABLE_COL_USER, user_id))))
            .bind(("disc", Thing::from((DISC_TABLE_NAME, disc_id))))
            .bind(("status", status))
            .await?;

        Ok(res.take::<Vec<T>>(0)?)
    }

    async fn get_by_user<T: for<'de> Deserialize<'de> + ViewFieldSelector + Send>(
        &self,
        user: &Thing,
        status: Option<TaskParticipantStatus>,
        is_ended: Option<bool>,
        pagination: Pagination,
    ) -> Result<Vec<T>, surrealdb::Error> {
        let order_dir = pagination.order_dir.unwrap_or(QryOrder::DESC).to_string();

        let date_condition = match is_ended {
            Some(value) => match value {
                true => "AND due_at <= time::now()",
                false => "AND due_at > time::now()",
            },
            None => "",
        };
        let status_condition = match &status {
            Some(_) => "AND status=$status",
            None => "",
        };
        let query = format!(
            "SELECT {} FROM {TASK_REQUEST_TABLE_NAME} WHERE ->task_participant[WHERE out=$user {}] {}
             ORDER BY created_at {order_dir} LIMIT $limit START $start;",
            &T::get_select_query_fields(),
            status_condition,
            date_condition,
        );
        let mut res = self
            .client
            .query(query)
            .bind(("user", user.clone()))
            .bind(("status", status))
            .bind(("limit", pagination.count))
            .bind(("start", pagination.start))
            .await?;

        Ok(res.take::<Vec<T>>(0)?)
    }

    async fn get_by_creator<T: for<'de> Deserialize<'de> + ViewFieldSelector + Send>(
        &self,
        user: Thing,
        pagination: Pagination,
    ) -> Result<Vec<T>, surrealdb::Error> {
        let order_dir = pagination.order_dir.unwrap_or(QryOrder::DESC).to_string();
        let fields = T::get_select_query_fields();
        let mut res = self
            .client
            .query(format!(
                "SELECT *, {fields} FROM {TASK_REQUEST_TABLE_NAME}
                 WHERE created_by=$user
                  AND belongs_to.type != $idea
                  AND (belongs_to.type == $public OR $user IN belongs_to<-{ACCESS_TABLE_NAME}.in)
                  AND (
                      record::tb(belongs_to) != {POST_TABLE_NAME}
                      OR belongs_to.belongs_to.type == $public
                      OR $user IN belongs_to.belongs_to<-{ACCESS_TABLE_NAME}.in
                  )
                 ORDER BY created_at {order_dir} LIMIT $limit START $start;"
            ))
            .bind(("user", user))
            .bind(("public", PostType::Public))
            .bind(("idea", PostType::Idea))
            .bind(("limit", pagination.count))
            .bind(("start", pagination.start))
            .await?;

        Ok(res.take::<Vec<T>>(0)?)
    }

    async fn update_status(
        &self,
        task_id: &str,
        status: TaskRequestStatus,
    ) -> Result<(), surrealdb::Error> {
        self.client
            .query("UPDATE $id SET status=$status;")
            .bind(("id", get_thing(task_id)?))
            .bind(("status", status))
            .await?
            .check()?;
        Ok(())
    }

    async fn get_ready_for_payment_by_id(
        &self,
        task_id: &str,
    ) -> Result<TaskForReward, surrealdb::Error> {
        let query = format!(
            "SELECT *, wallet.transaction_head[currency].balance as balance
             FROM (
                SELECT id, wallet_id.* AS wallet, currency, request_txt, belongs_to,
                    ->task_participant.{{ status, id, user: out.* }} AS participants,
                    ->task_donor.{{ id: out, amount: transaction.amount_out }} AS donors
                FROM $task WHERE status != $status
            )"
        );
        let mut res = self
            .client
            .query(query)
            .bind(("task", get_thing(task_id)?))
            .bind(("status", TaskRequestStatus::Completed))
            .await?;

        let data = res.take::<Option<TaskForReward>>(0)?;
        data.ok_or(surrealdb::Error::Db(surrealdb::error::Db::IdNotFound {
            rid: task_id.to_string(),
        }))
    }

    async fn get_ready_for_payment(&self) -> Result<Vec<TaskForReward>, surrealdb::Error> {
        let query = format!(
            "SELECT *, wallet.transaction_head[currency].balance as balance
             FROM (
                SELECT id, wallet_id.* AS wallet, currency, request_txt, belongs_to,
                    ->task_participant.{{ status, id, user: out.* }} AS participants,
                    ->task_donor.{{ id: out, amount: transaction.amount_out }} AS donors
                FROM {TASK_REQUEST_TABLE_NAME}
                WHERE status != $status AND due_at <= time::now()
            )"
        );
        let mut res = self
            .client
            .query(query)
            .bind(("status", TaskRequestStatus::Completed))
            .await?;
        let data = res.take::<Vec<TaskForReward>>(0)?;
        Ok(data)
    }

    async fn get_by_id<T: for<'de> Deserialize<'de> + ViewFieldSelector + Send>(
        &self,
        id: &str,
    ) -> AppResult<T> {
        let fields = T::get_select_query_fields();
        let query = format!("SELECT {fields} FROM $task;");
        let mut res = self
            .client
            .query(query)
            .bind(("task", get_thing(id)?))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;

        let data = res.take::<Option<T>>(0).map_err(|e| AppError::SurrealDb {
            source: e.to_string(),
        })?;

        data.ok_or(AppError::EntityFailIdNotFound {
            ident: id.to_string(),
        })
    }
}
