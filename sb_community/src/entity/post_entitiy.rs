use serde::{Deserialize, Serialize};
use surrealdb::err::Error::IndexExists;
use surrealdb::opt::PatchOp;
use surrealdb::sql::{Id, Thing};
use surrealdb::Error as ErrorSrl;
use validator::Validate;

use crate::entity::reply_entitiy::Reply;
use sb_middleware::db;
use sb_middleware::utils::db_utils::{
    exists_entity, get_entity, get_entity_list_view, get_entity_view, with_not_found_err,
    IdentIdName, Pagination, QryOrder, ViewFieldSelector,
};
use sb_middleware::utils::extractor_utils::DiscussionParams;
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};

#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
pub struct Post {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub belongs_to: Thing,
    pub created_by: Thing,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discussion_topic: Option<Thing>,
    // #[serde(skip_serializing)]
    pub r_title_uri: Option<String>,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_links: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Vec<String>>,
    // #[serde(skip_serializing)]
    pub r_created: Option<String>,
    // #[serde(skip_serializing)]
    pub r_updated: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_replies: Option<Vec<Reply>>,
    pub replies_nr: i64,
    pub likes_nr: i64,
}

pub struct PostDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "post";
// origin
const TABLE_COL_DISCUSSION: &str = crate::entity::discussion_entitiy::TABLE_NAME;
const TABLE_COL_TOPIC: &str = crate::entity::discussion_topic_entitiy::TABLE_NAME;
const TABLE_COL_USER: &str = sb_user_auth::entity::local_user_entity::TABLE_NAME;
const INDEX_BELONGS_TO_URI: &str = "belongs_to_x_title_uri_idx";

impl<'a> PostDbService<'a> {
    pub fn get_table_name() -> &'static str {
        TABLE_NAME
    }

    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD belongs_to ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_DISCUSSION}>;
    DEFINE FIELD created_by ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE FIELD title ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD r_title_uri ON TABLE {TABLE_NAME} VALUE string::slug($this.title);
    DEFINE INDEX {INDEX_BELONGS_TO_URI} ON TABLE {TABLE_NAME} COLUMNS belongs_to, r_title_uri UNIQUE;
    DEFINE FIELD {TABLE_COL_TOPIC} ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_COL_TOPIC}>>
        ASSERT $value INSIDE (SELECT topics FROM ONLY $this.belongs_to).topics;
    DEFINE INDEX {TABLE_COL_TOPIC}_idx ON TABLE {TABLE_NAME} COLUMNS {TABLE_COL_TOPIC};
    DEFINE FIELD content ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD media_links ON TABLE {TABLE_NAME} TYPE option<array<string>>;
    DEFINE FIELD metadata ON TABLE {TABLE_NAME} TYPE option<set<string>>;
    DEFINE FIELD replies_nr ON TABLE {TABLE_NAME} TYPE number DEFAULT 0;
    DEFINE FIELD likes_nr ON TABLE {TABLE_NAME} TYPE number DEFAULT 0;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();

");
        let mutation = self.db.query(sql).await?;
        mutation.check().expect("should mutate domain");

        Ok(())
    }

    pub async fn must_exist(&self, ident: IdentIdName) -> CtxResult<Thing> {
        let opt = exists_entity(self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, ident.to_string().as_str())
    }

    pub async fn get(&self, ident_id_name: IdentIdName) -> CtxResult<Post> {
        let opt = get_entity::<Post>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn get_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        ident_id_name: IdentIdName,
    ) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    /*pub async fn get_by_discussion_desc(&self, discussionId: Thing, topic: Option<String>, from: i32, count: i8) -> ApiResult<Vec<Post>> {
        let filter_by = Self::create_filter(discussionId, topic);
        get_entity_list::<Post>(self.db, TABLE_NAME.to_string(), &filter_by,
                                Some(Pagination { order_by: Option::from("r_created".to_string()), order_dir: Some(QryOrder::DESC), limit: 20, start: 0 }
                                )).await
    }*/

    pub async fn get_by_discussion_desc_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        discussion_id: Thing,
        params: DiscussionParams,
    ) -> CtxResult<Vec<T>> {
        let filter_by = Self::create_filter(discussion_id, params.topic_id);
        let pagination = Some(Pagination {
            order_by: Option::from("r_created".to_string()),
            order_dir: Some(QryOrder::DESC),
            count: params.count.unwrap_or(20),
            start: params.start.unwrap_or(0),
        });
        get_entity_list_view::<T>(self.db, TABLE_NAME.to_string(), &filter_by, pagination).await
    }

    fn create_filter(discussion_id: Thing, topic: Option<Thing>) -> IdentIdName {
        let filter_discussion = IdentIdName::ColumnIdent {
            column: "belongs_to".to_string(),
            val: discussion_id.to_raw(),
            rec: true,
        };
        let filter_by = match topic {
            None => filter_discussion,
            Some(topic_val) => IdentIdName::ColumnIdentAnd(vec![
                filter_discussion,
                IdentIdName::ColumnIdent {
                    column: TABLE_COL_TOPIC.to_string(),
                    val: topic_val.to_raw(),
                    rec: true,
                },
            ]),
        };
        filter_by
    }

    pub async fn create(&self, mut record: Post) -> CtxResult<Post> {
        record.id = Some(Thing::from((TABLE_NAME, Id::ulid())));
        self.db
            .create(TABLE_NAME)
            .content(record)
            .await
            .map_err(|e| match e {
                ErrorSrl::Db(err) => match err {
                    IndexExists { index, .. } if index == INDEX_BELONGS_TO_URI => {
                        self.ctx.to_ctx_error(AppError::Generic {
                            description: "Title already exists".to_string(),
                        })
                    }
                    _ => CtxError::from(self.ctx)(ErrorSrl::Db(err)),
                },
                _ => CtxError::from(self.ctx)(e),
            })
            .map(|v: Option<Post>| v.unwrap())
    }

    pub async fn set_media_url(&self, post_id: Thing, url: &str) -> CtxResult<Post> {
        // TODO add index para to change particular url
        let res: Option<Post> = self
            .db
            .update((post_id.tb.clone(), post_id.id.clone().to_string()))
            .patch(PatchOp::add("/media_links", [url]))
            .await
            .map_err(CtxError::from(self.ctx))?;
        res.ok_or_else(|| {
            self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
                ident: post_id.to_raw(),
            })
        })
    }

    pub async fn increase_replies_nr(&self, record: Thing) -> CtxResult<Post> {
        let curr_nr = self
            .db
            .query("SELECT replies_nr FROM <record>$rec".to_string())
            .bind(("rec", record.clone().to_raw()))
            .await?
            .take::<Option<i64>>("replies_nr")?
            .ok_or_else(|| {
                self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
                    ident: record.clone().to_raw(),
                })
            })?;

        let res: Option<Post> = self
            .db
            .update((record.tb.clone(), record.id.clone().to_raw()))
            .patch(PatchOp::replace("/replies_nr", curr_nr + 1))
            .await
            .map_err(CtxError::from(self.ctx))?;
        res.ok_or_else(|| {
            self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
                ident: record.to_raw(),
            })
        })
    }
}
