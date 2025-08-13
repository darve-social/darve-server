use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::err::Error::IndexExists;
use surrealdb::opt::PatchOp;
use surrealdb::sql::{Id, Thing};
use surrealdb::Error as ErrorSrl;
use validator::Validate;

use crate::database::table_names::{TAG_REL_TABLE_NAME, TAG_TABLE_NAME};
use middleware::utils::db_utils::{
    exists_entity, get_entity, get_entity_list_view, get_entity_view, with_not_found_err,
    IdentIdName, Pagination, QryOrder, ViewFieldSelector,
};
use middleware::utils::extractor_utils::DiscussionParams;
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};

use crate::database::client::Db;
use crate::entities::user_auth::local_user_entity;
use crate::middleware;
use crate::middleware::utils::string_utils::get_str_thing;
use crate::services::post_service::PostView;

use super::discussion_entity;

#[derive(Debug, Deserialize, Serialize)]
pub enum PostDenyRule {
    CreateTask,
}

/// Post belongs_to discussion.
/// It is created_by user. Since user can create posts in different
/// discussions we have to filter by discussion and user to get user's posts
/// in particular discussion. User profile posts are in profile discussion.
///
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct Post {
    // id is ULID for sorting by time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub belongs_to: Thing,
    pub created_by: Thing,
    pub title: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_links: Option<Vec<String>>,
    #[serde(default)]
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub replies_nr: i64,
    pub likes_nr: i64,
    pub tags: Option<Vec<String>>,
    pub deny_rules: Option<Vec<PostDenyRule>>,
}
#[derive(Debug, Serialize)]
pub struct CreatePost {
    pub id: Thing,
    pub belongs_to: Thing,
    pub created_by: Thing,
    pub title: String,
    pub content: Option<String>,
    pub media_links: Option<Vec<String>>,
}

pub struct PostDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "post";

// origin
const TABLE_COL_DISCUSSION: &str = discussion_entity::TABLE_NAME;
const TABLE_COL_USER: &str = local_user_entity::TABLE_NAME;
const TABLE_COL_BELONGS_TO: &str = "belongs_to";
const INDEX_BELONGS_TO_URI: &str = "belongs_to_x_title_uri_idx";
const INDEX_BELONGS_TO: &str = "belongs_to_idx";

impl<'a> PostDbService<'a> {
    pub fn get_table_name() -> &'static str {
        TABLE_NAME
    }

    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS {TABLE_COL_BELONGS_TO} ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_DISCUSSION}>;
    DEFINE INDEX IF NOT EXISTS {INDEX_BELONGS_TO} ON TABLE {TABLE_NAME} COLUMNS {TABLE_COL_BELONGS_TO};
    DEFINE FIELD IF NOT EXISTS created_by ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE FIELD IF NOT EXISTS title ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD IF NOT EXISTS content ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS media_links ON TABLE {TABLE_NAME} TYPE option<array<string>>;
    DEFINE FIELD IF NOT EXISTS metadata ON TABLE {TABLE_NAME} TYPE option<set<string>>;
    DEFINE FIELD IF NOT EXISTS replies_nr ON TABLE {TABLE_NAME} TYPE number DEFAULT 0;
    DEFINE FIELD IF NOT EXISTS likes_nr ON TABLE {TABLE_NAME} TYPE number DEFAULT 0;
    DEFINE FIELD IF NOT EXISTS deny_rules ON TABLE {TABLE_NAME} TYPE option<set<string>>;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS updated_at ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE time::now();
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

    pub async fn get_by_id(&self, id: &str) -> CtxResult<Post> {
        let thing = get_str_thing(&id)?;
        let ident = IdentIdName::Id(thing);
        self.get(ident).await
    }

    pub async fn get_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        ident_id_name: IdentIdName,
    ) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn get_by_query(
        &self,
        user_id: &str,
        disc_id: &str,
        pag: Pagination,
    ) -> CtxResult<Vec<PostView>> {
        let order_dir = pag.order_dir.unwrap_or(QryOrder::DESC).to_string();

        let query = format!(
            "SELECT {} FROM {TABLE_NAME}
                WHERE belongs_to == $disc
                ORDER BY id {order_dir} LIMIT $limit START $start;",
            PostView::get_select_query_fields()
        );

        let mut res = self
            .db
            .query(query)
            .bind(("limit", pag.count))
            .bind(("start", pag.start))
            .bind(("disc", Thing::from((TABLE_COL_DISCUSSION, disc_id))))
            .bind(("user", Thing::from((TABLE_COL_USER, user_id))))
            .await?;

        let posts = res.take::<Vec<PostView>>(0)?;

        Ok(posts)
    }

    pub async fn get_by_discussion_desc_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        discussion_id: Thing,
        params: DiscussionParams,
    ) -> CtxResult<Vec<T>> {
        let filter_by = IdentIdName::ColumnIdent {
            column: TABLE_COL_BELONGS_TO.to_string(),
            val: discussion_id.to_raw(),
            rec: true,
        };
        let pagination = Some(Pagination {
            // id is ULID so can be ordered by time
            order_by: Some("id".to_string()),
            order_dir: Some(QryOrder::DESC),
            count: params.count.unwrap_or(20),
            start: params.start.unwrap_or(0),
        });
        get_entity_list_view::<T>(self.db, TABLE_NAME.to_string(), &filter_by, pagination).await
    }

    pub async fn get_by_id_with_access(&self, post_id: &str) -> CtxResult<Post> {
        let mut res = self
            .db
            .query("SELECT * FROM $id WHERE $access_rule;")
            .bind(("id", get_str_thing(post_id)?))
            .bind(("access_rule", self.get_access_query()))
            .await?;
        let post = res.take::<Option<Post>>(0)?;

        post.ok_or(
            AppError::EntityFailIdNotFound {
                ident: post_id.to_string(),
            }
            .into(),
        )
    }

    pub async fn get_by_tag(&self, tag: &str, pagination: Pagination) -> CtxResult<Vec<Post>> {
        let order_dir = pagination.order_dir.unwrap_or(QryOrder::DESC).to_string();
        let order_by = pagination.order_by.unwrap_or("id".to_string()).to_string();

        let query = format!(
            "SELECT *, out.* AS entity FROM $tag->{TAG_REL_TABLE_NAME}
             WHERE record::id(out.belongs_to)=record::id(out.created_by)
             ORDER BY out.{} {} LIMIT $limit START $start;",
            order_by, order_dir
        );
        let mut res = self
            .db
            .query(query)
            .bind(("tag", Thing::from((TAG_TABLE_NAME, tag))))
            .bind(("limit", pagination.count))
            .bind(("start", pagination.start))
            .await?;

        let posts = res.take::<Vec<Post>>((0, "entity"))?;
        Ok(posts)
    }

    fn get_access_query(&self) -> String {
        "record::id($this.belongs_to)=record::id($this.created_by)
        AND record::exists(type::record(string::concat('access_rule:',record::id($this.id))) )=false
        AND record::exists(type::record(string::concat('access_rule:',record::id($this.belongs_to))) )=false
        AND record::exists(type::record(string::concat('access_rule:',record::id($this.belongs_to.belongs_to))) )=false
        ".to_string()
    }

    pub async fn create(&self, data: CreatePost) -> CtxResult<Post> {
        self.db
            .create(TABLE_NAME)
            .content(data)
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

    pub fn get_new_post_thing() -> Thing {
        // id is ULID for sorting by time
        Thing::from((TABLE_NAME.to_string(), Id::ulid()))
    }
}
