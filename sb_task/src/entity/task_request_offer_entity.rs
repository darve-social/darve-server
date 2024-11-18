use sb_middleware::db;
use sb_middleware::utils::db_utils::{get_entity, get_entity_list, with_not_found_err, IdentIdName};
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use sb_user_auth::entity::access_rule_entity::AccessRule;
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};
use surrealdb::opt::PatchOp;
use surrealdb::sql::{Id, Thing};
use sb_community::entity::discussion_topic_entitiy::DiscussionTopic;
use crate::entity::task_request_entitiy::TaskRequestDbService;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRequestOffer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub task_request: Thing,
    pub user: Thing,
    pub amount: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

pub struct TaskRequestOfferDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "task_request_offer";
const TABLE_COL_USER: &str = sb_user_auth::entity::local_user_entity::TABLE_NAME;
const TABLE_COL_TASK_REQUEST: &str = crate::entity::task_request_entitiy::TABLE_NAME;

impl<'a> TaskRequestOfferDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {

        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD user ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE INDEX user_idx ON TABLE {TABLE_NAME} COLUMNS user;
    DEFINE FIELD task_request ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_TASK_REQUEST}>;
    DEFINE INDEX user_treq_idx ON TABLE {TABLE_NAME} COLUMNS user, task_request UNIQUE;
    DEFINE FIELD amount ON TABLE {TABLE_NAME} TYPE number;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.db
            .query(sql)
            .await?;

        &mutation.check().expect("should mutate taskRequestOffer");

        Ok(())
    }

    pub async fn add_to_task_offers(&self, task_request: Thing, user: Thing, amount: i64) -> CtxResult<TaskRequestOffer> {
        // get task offer or create
        let offer = self.get(IdentIdName::ColumnIdentAnd(vec![
            IdentIdName::ColumnIdent {
                column: "user".to_string(),
                val: user.to_raw(),
                rec: true,
            },
            IdentIdName::ColumnIdent {
                column: "task_request".to_string(),
                val: task_request.to_raw(),
                rec: true,
            }
        ])).await;
        let existing_offer_id = match offer.ok() {
            None => None,
            Some(offer) => offer.id
        };
        let offer = self.create_update(TaskRequestOffer{
            id: existing_offer_id.clone(),
            task_request: task_request.clone(),
            user,
            amount,
            r_created: None,
            r_updated: None,
        }).await?;

        if existing_offer_id.is_none() {
            TaskRequestDbService{db: self.db, ctx: self.ctx}.add_offer(task_request, offer.id.clone().unwrap()).await?;
        }

        Ok(offer)
    }

    pub async fn create_update(&self, mut record: TaskRequestOffer) -> CtxResult<TaskRequestOffer> {
        let resource = record.id.clone().unwrap_or(Thing::from((TABLE_NAME.to_string(), Id::rand() )));
        record.r_created = None;
        record.r_updated = None;

        self.db
            .upsert( (resource.tb, resource.id.to_raw()))
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))
        .map(|v: Option<TaskRequestOffer>| v.unwrap())

    }

    pub async fn get(&self, ident: IdentIdName) -> CtxResult<TaskRequestOffer> {
        let opt = get_entity::<TaskRequestOffer>(&self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, &ident.to_string().as_str())
    }

}

