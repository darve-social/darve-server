use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use surrealdb::opt::PatchOp;
use surrealdb::sql::Thing;
use sb_community::entity::post_entitiy::Post;
use sb_middleware::{
    ctx::Ctx,
    error::{CtxError, CtxResult, AppError},
};
use sb_middleware::db;
use sb_middleware::utils::db_utils::{get_entity_list_view, get_entity_view, IdentIdName, Pagination, QryOrder, ViewFieldSelector, with_not_found_err, get_entity, get_entity_list, exists_entity};
use sb_user_auth::entity::access_rule_entity::AccessRule;
use sb_user_auth::entity::local_user_entity::LocalUser;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub from_user: Thing,
    pub to_user: Thing,
    pub post: Option<Thing>,
    pub content: String,
    pub offer_amount: u64,
    pub status: String,
    // #[serde(skip_serializing)]
    pub r_created: Option<String>,
    // #[serde(skip_serializing)]
    pub r_updated: Option<String>,
}

#[derive(EnumString, Display)]
pub enum TaskStatus {
    Requested,
    Accepted,
    Rejected,
    Delivered,
    // Complete,
}

pub struct TaskRequestDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "task_request";
const TABLE_COL_POST: &str = sb_community::entity::post_entitiy::TABLE_NAME;
const TABLE_COL_USER: &str = sb_user_auth::entity::local_user_entity::TABLE_NAME;

impl<'a> TaskRequestDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let t_stat_req = TaskStatus::Requested.to_string();
        let t_stat_acc = TaskStatus::Accepted.to_string();
        let t_stat_rej = TaskStatus::Rejected.to_string();
        let t_stat_del = TaskStatus::Delivered.to_string();

        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD post ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_COL_POST}>>;
    DEFINE INDEX post_idx ON TABLE {TABLE_NAME} COLUMNS post;
    DEFINE FIELD from_user ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE INDEX from_user_idx ON TABLE {TABLE_NAME} COLUMNS from_user;
    DEFINE FIELD to_user ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE INDEX to_user_idx ON TABLE {TABLE_NAME} COLUMNS to_user;
    DEFINE FIELD content ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD status ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0
        ASSERT $value INSIDE ['{t_stat_req}','{t_stat_acc}','{t_stat_rej},'{t_stat_del}'];
    DEFINE INDEX status_idx ON TABLE {TABLE_NAME} COLUMNS status;
    DEFINE FIELD offer_amount ON TABLE {TABLE_NAME} TYPE number;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE INDEX r_created_idx ON TABLE {TABLE_NAME} COLUMNS r_created;
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.db
            .query(sql)
            .await?;

        &mutation.check().expect("should mutate taskRequest");

        Ok(())
    }

    pub async fn create(&self, record: TaskRequest) -> CtxResult<TaskRequest> {
        let res = self.db
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

    pub async fn to_user_post_list(&self, to_user: Thing, post_id: Thing) -> CtxResult<Vec<AccessRule>> {
        get_entity_list::<AccessRule>(self.db, TABLE_NAME.to_string(),
                                      &IdentIdName::ColumnIdentAnd(vec![
                                          IdentIdName::ColumnIdent {
                                              column: "to_user".to_string(),
                                              val: to_user.to_raw(),
                                              rec: true
                                          },
                                          IdentIdName::ColumnIdent {
                                              column: "post".to_string(),
                                              val: post_id.to_raw(),
                                              rec: true
                                          },
                                      ]), None).await
    }

    pub async fn from_user_post_list(&self, to_user: Thing, post_id: Thing) -> CtxResult<Vec<AccessRule>> {
        get_entity_list::<AccessRule>(self.db, TABLE_NAME.to_string(), &IdentIdName::ColumnIdentAnd(vec![
            IdentIdName::ColumnIdent {
                column: "to_user".to_string(),
                val: to_user.to_raw(),
                rec: true
            },
            IdentIdName::ColumnIdent {
                column: "post".to_string(),
                val: post_id.to_raw(),
                rec: true
            },
        ]), None).await
    }

    pub async fn to_user_status_list(&self, to_user: Thing, status: TaskStatus) -> CtxResult<Vec<AccessRule>> {
        get_entity_list::<AccessRule>(self.db, TABLE_NAME.to_string(), &IdentIdName::ColumnIdentAnd(vec![
            IdentIdName::ColumnIdent {
                column: "to_user".to_string(),
                val: to_user.to_raw(),
                rec: true
            },
            IdentIdName::ColumnIdent {
                column: "status".to_string(),
                val: status.to_string(),
                rec: false
            },
        ]), None).await
    }

    pub async fn update_status(&self, user: Thing, record: Thing, status: TaskStatus) -> CtxResult<TaskRequest> {
        let task = self.get(IdentIdName::Id(record.clone().to_raw())).await?;
        if task.to_user == user {
            let res: Option<TaskRequest> = self.db.update((record.tb.clone(), record.id.clone().to_raw()))
                .patch(PatchOp::replace("/status", status.to_string()))
                .await
                .map_err(CtxError::from(self.ctx))?;
            res.ok_or_else(|| self.ctx.to_ctx_error(AppError::EntityFailIdNotFound { ident: record.to_raw() }))
        } else {
            Err(self.ctx.to_ctx_error(AppError::AuthorizationFail { required: "Task set to user".to_string() }))
        }
    }
}

