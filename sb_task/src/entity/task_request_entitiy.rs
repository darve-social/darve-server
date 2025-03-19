use sb_middleware::db;
use sb_middleware::utils::db_utils::{get_entity, get_entity_list, get_entity_list_view, with_not_found_err, IdentIdName, Pagination, ViewFieldSelector};
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use surrealdb::opt::PatchOp;
use surrealdb::sql::Thing;
use crate::entity::task_deliverable_entitiy::{TaskDeliverable, TaskDeliverableDbService};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub from_user: Thing,
    pub to_user: Option<Thing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_post: Option<Thing>,
    pub request_txt: String,
    pub deliverable_type: DeliverableType,
    // TODO looks like offers could just be single TaskRequestOffer since there is an array of ParticipantRewards in it for different user amount participations 
    pub offers: Vec<Thing>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deliverables: Option<Vec<Thing>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_updated: Option<String>,
}

#[derive(EnumString, Display)]
pub enum TaskStatus {
    Requested,
    Accepted,
    Rejected,
    Delivered,
    Complete,
}

#[derive(EnumString, Display, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DeliverableType {
    PublicPost,
    // Participants,
}

#[derive(Display)]
pub enum UserTaskRole {
    // needs to be same as col name
    #[strum(to_string = "from_user")]
    FromUser, // created task
    #[strum(to_string = "to_user")]
    ToUser, // received task
}

pub struct TaskRequestDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

impl<'a> TaskRequestDbService<'a> {}

pub const TABLE_NAME: &str = "task_request";
const TABLE_COL_POST: &str = sb_community::entity::post_entitiy::TABLE_NAME;
const TABLE_COL_USER: &str = sb_user_auth::entity::local_user_entity::TABLE_NAME;
const TABLE_COL_OFFER: &str = crate::entity::task_request_offer_entity::TABLE_NAME;
const TABLE_COL_DELIVERABLE: &str = crate::entity::task_deliverable_entitiy::TABLE_NAME;

impl<'a> TaskRequestDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let t_stat_req = TaskStatus::Requested.to_string();
        let t_stat_acc = TaskStatus::Accepted.to_string();
        let t_stat_rej = TaskStatus::Rejected.to_string();
        let t_stat_del = TaskStatus::Delivered.to_string();
        let t_stat_com = TaskStatus::Complete.to_string();

        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD request_post ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_COL_POST}>>;
    DEFINE INDEX request_post_idx ON TABLE {TABLE_NAME} COLUMNS request_post;
    DEFINE FIELD from_user ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE INDEX from_user_idx ON TABLE {TABLE_NAME} COLUMNS from_user;
    DEFINE FIELD to_user ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_COL_USER}>>;
    DEFINE INDEX to_user_idx ON TABLE {TABLE_NAME} COLUMNS to_user;
    DEFINE FIELD deliverable_type ON TABLE {TABLE_NAME} TYPE {{ type: \"PublicPost\"}};
    DEFINE FIELD request_txt ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD status ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0
        ASSERT $value INSIDE ['{t_stat_req}','{t_stat_acc}','{t_stat_rej}','{t_stat_del}','{t_stat_com}'];
    DEFINE INDEX status_idx ON TABLE {TABLE_NAME} COLUMNS status;
    DEFINE FIELD offers ON TABLE {TABLE_NAME} TYPE set<record<{TABLE_COL_OFFER}>>;
    DEFINE FIELD deliverables ON TABLE {TABLE_NAME} TYPE option<set<record<{TABLE_COL_DELIVERABLE}>>>;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    //DEFINE INDEX r_created_idx ON TABLE {TABLE_NAME} COLUMNS r_created;
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.db.query(sql).await?;

        &mutation.check().expect("should mutate taskRequest");

        Ok(())
    }

    pub async fn create(&self, record: TaskRequest) -> CtxResult<TaskRequest> {
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

    /*pub(crate) async fn add_offer(
        &self,
        task_id: Thing,
        offer_id: Thing,
    ) -> CtxResult<TaskRequest> {
        let res: Option<TaskRequest> = self
            .db
            .update((task_id.tb.clone(), task_id.id.clone().to_string()))
            .patch(PatchOp::add("/offers", [offer_id.clone()]))
            .await
            .map_err(CtxError::from(self.ctx))?;
        res.ok_or_else(|| {
            self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
                ident: offer_id.to_raw(),
            })
        })
    }*/

    pub async fn user_post_list_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        user_task_role: UserTaskRole,
        user: Thing,
        post_id: Option<Thing>,
        pagination: Option<Pagination>,
    ) -> CtxResult<Vec<T>> {
        let mut filter_by = vec![
            IdentIdName::ColumnIdent {
                column: user_task_role.to_string(),
                val: user.to_raw(),
                rec: true,
            },
        ];
        if post_id.is_some() {
            filter_by.push(
                IdentIdName::ColumnIdent {
                    column: "request_post".to_string(),
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

    pub async fn request_post_list_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(&self, post_id: Thing) -> CtxResult<Vec<T>> {
        get_entity_list_view::<T>(
            self.db,
            TABLE_NAME.to_string(),
            &IdentIdName::ColumnIdent {
                column: "request_post".to_string(),
                val: post_id.to_raw(),
                rec: true,
            },
            None,
        )
            .await
    }

    pub async fn user_status_list(
        &self,
        user_task_role: UserTaskRole,
        user: Thing,
        status: TaskStatus,
    ) -> CtxResult<Vec<TaskRequest>> {
        get_entity_list::<TaskRequest>(
            self.db,
            TABLE_NAME.to_string(),
            &IdentIdName::ColumnIdentAnd(vec![
                IdentIdName::ColumnIdent {
                    column: user_task_role.to_string(),
                    val: user.to_raw(),
                    rec: true,
                },
                IdentIdName::ColumnIdent {
                    column: "status".to_string(),
                    val: status.to_string(),
                    rec: false,
                },
            ]),
            None,
        )
            .await
    }

    pub async fn update_status_received_by_user(
        &self,
        user: Thing,
        task_ident: Thing,
        status: TaskStatus,
        delivered_urls: Option<Vec<String>>,
        delivered_post: Option<Thing>,
    ) -> CtxResult<(TaskRequest, Option<Thing>)> {
        let task = self.get(IdentIdName::Id(task_ident.clone())).await?;
        if task.to_user.is_none() || task.to_user.clone().expect("is some") == user {
            let update_op = self
                .db
                .update((task_ident.tb.clone(), task_ident.id.clone().to_raw()));

            let (update_op, deliverable_id) = match status {
                TaskStatus::Delivered => {

                    if delivered_urls.is_none() && delivered_post.is_none() {
                        return Err(self.ctx.to_ctx_error(AppError::Generic {
                            description: "Deliverable empty".to_string(),
                        }));
                    }
                    let deliverables_service = TaskDeliverableDbService { db: self.db, ctx: self.ctx };
                    let deliverable_id = deliverables_service.create(TaskDeliverable{
                            id: None,
                            user,
                            task_request: task_ident.clone(),
                        urls: delivered_urls,
                        post: delivered_post,
                            r_created: None,
                            r_updated: None,
                        }).await?.id.expect("is saved");
                    let update_op = update_op
                        .patch(PatchOp::replace("/status", status.to_string()))
                        .patch(PatchOp::add("/deliverables/-", [deliverable_id.clone()]));
                    (update_op, Some(deliverable_id))
                }
                _ => {
                    if task.to_user.is_some(){
                        (update_op.patch(PatchOp::replace("/status", status.to_string())), None)
                    }else {
                        return Err(self.ctx.to_ctx_error(AppError::AuthorizationFail {
                            required:"User needs task privileges to update status".to_string(),
                        }));
                    }
                },
            };

            let res: Option<TaskRequest> = update_op.await.map_err(CtxError::from(self.ctx))?;
            let task = res.ok_or_else(|| {
                self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
                    ident: task_ident.to_raw(),
                })
            })?;
            Ok((task, deliverable_id))
        } else {
            Err(self.ctx.to_ctx_error(AppError::AuthorizationFail {
                required: "Task set to user".to_string(),
            }))
        }
    }
}
