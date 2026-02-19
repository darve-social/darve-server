use crate::database::table_names::{
    ACCESS_TABLE_NAME, TAG_REL_TABLE_NAME, TAG_TABLE_NAME, TASK_REQUEST_TABLE_NAME,
};
use crate::entities::community::discussion_entity::DiscussionType;
use crate::middleware::error::AppResult;
use crate::middleware::utils::db_utils::{CursorPagination, Pagination, ViewRelateField};
use chrono::{DateTime, Utc};
use middleware::utils::db_utils::{QryOrder, ViewFieldSelector};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use serde::{Deserialize, Serialize, Serializer};
use surrealdb::types::{RecordId, RecordIdKey, SurrealValue};
use validator::Validate;

use crate::database::client::Db;
use crate::entities::user_auth::follow_entity::TABLE_NAME as FOLLOW_TABLE_NAME;
use crate::entities::user_auth::local_user_entity;
use crate::middleware;
use crate::middleware::utils::string_utils::get_str_thing;
use crate::models::view::post::PostView;

use super::discussion_entity;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone, SurrealValue)]
pub enum PostType {
    Public,
    Private,
    Idea,
}

#[derive(Debug, PartialEq, Eq, Clone, SurrealValue)]
pub enum PostUserStatus {
    Delivered,
    Seen,
}

impl Serialize for PostUserStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(self.clone() as u8)
    }
}
impl<'de> Deserialize<'de> for PostUserStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = u8::deserialize(deserializer)?;
        match v {
            0 => Ok(PostUserStatus::Delivered),
            1 => Ok(PostUserStatus::Seen),
            _ => Err(serde::de::Error::custom(format!(
                "invalid PostUserStatus value: {}",
                v
            ))),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct Post {
    // id is ULID for sorting by time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RecordId>,
    pub belongs_to: RecordId,
    pub created_by: RecordId,
    pub title: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_links: Option<Vec<String>>,
    #[serde(default)]
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub replies_nr: i64,
    pub tasks_nr: u64,
    pub likes_nr: i64,
    pub r#type: PostType,
    pub reply_to: Option<RecordId>,
}

#[derive(Debug, Serialize, SurrealValue)]
pub struct CreatePost {
    pub id: RecordId,
    pub belongs_to: RecordId,
    pub created_by: RecordId,
    pub title: String,
    pub content: Option<String>,
    pub media_links: Option<Vec<String>>,
    pub r#type: PostType,
    pub delivered_for_task: Option<RecordId>,
    pub reply_to: Option<RecordId>,
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
    DEFINE FIELD IF NOT EXISTS tasks_nr ON TABLE {TABLE_NAME} TYPE number DEFAULT 0;
    DEFINE FIELD IF NOT EXISTS type ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD IF NOT EXISTS reply_to ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_NAME}>>;
    DEFINE FIELD IF NOT EXISTS delivered_for_task ON TABLE {TABLE_NAME} TYPE option<record<{TASK_REQUEST_TABLE_NAME}>>;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS updated_at ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE time::now();
    DEFINE INDEX IF NOT EXISTS idx_type ON TABLE {TABLE_NAME} COLUMNS type;
    DEFINE INDEX IF NOT EXISTS idx_title ON {TABLE_NAME} FIELDS title;
");
        let mutation = self.db.query(sql).await?;
        mutation.check().expect("should mutate domain");

        Ok(())
    }

    pub async fn get_view_by_id<T: for<'b> Deserialize<'b> + SurrealValue + ViewFieldSelector>(
        &self,
        post_id: &str,
        current_user_id: Option<&str>,
    ) -> CtxResult<T> {
        let fields = T::get_select_query_fields();
        let user = current_user_id.map(|id| RecordId::new(TABLE_COL_USER, id));
        let mut res = self
            .db
            .query(format!("SELECT {fields} FROM $post"))
            .bind(("post", get_str_thing(post_id)?))
            .bind(("user", user))
            .await?;

        let data = res
            .take::<Option<T>>(0)?
            .ok_or(AppError::EntityFailIdNotFound {
                ident: post_id.to_string(),
            })?;

        Ok(data)
    }

    pub async fn get_by_disc(
        &self,
        user_id: &str,
        disc_id: &str,
        filter_by_type: Option<PostType>,
        pag: CursorPagination,
    ) -> CtxResult<Vec<PostView>> {
        let order_dir = pag.order_dir.to_string();
        let query_by_type = match filter_by_type {
            Some(_) => "AND type=$filter_by_type",
            None => "",
        };

        let query_by_id = match pag.cursor {
            Some(_) => match pag.order_dir {
                QryOrder::DESC => "AND id < $cursor",
                _ => "AND id > $cursor",
            },
            None => "",
        };

        let fields = PostView::get_select_query_fields();

        let query = format!(
            "SELECT {fields} FROM {TABLE_NAME}
            WHERE belongs_to=$disc {query_by_id} {query_by_type} AND (type IN $public_post_types OR $user IN <-{ACCESS_TABLE_NAME}.in)
            ORDER BY id {order_dir} LIMIT $limit;"
        );

        let mut res = self
            .db
            .query(query)
            .bind(("limit", pag.count))
            .bind(("cursor", pag.cursor))
            .bind(("filter_by_type", filter_by_type))
            .bind(("public_post_types", vec![PostType::Public, PostType::Idea]))
            .bind(("disc", RecordId::new(TABLE_COL_DISCUSSION, disc_id)))
            .bind(("user", RecordId::new(TABLE_COL_USER, user_id)))
            .await?;

        let posts = res.take::<Vec<PostView>>(0)?;

        Ok(posts)
    }

    pub async fn get_count(
        &self,
        user_id: &str,
        disc_id: &str,
        filter_by_type: Option<PostType>,
    ) -> CtxResult<u64> {
        let query_by_type = match filter_by_type {
            Some(_) => "AND type=$filter_by_type",
            None => "",
        };
        let query = format!(
            "count(SELECT id FROM {TABLE_NAME} WHERE
                belongs_to=$disc {query_by_type}
                AND (type IN $public_post_types OR $user IN <-{ACCESS_TABLE_NAME}.in)
            )"
        );

        let mut res = self
            .db
            .query(query)
            .bind(("filter_by_type", filter_by_type))
            .bind(("public_post_types", vec![PostType::Public, PostType::Idea]))
            .bind(("disc", RecordId::new(TABLE_COL_DISCUSSION, disc_id)))
            .bind(("user", RecordId::new(TABLE_COL_USER, user_id)))
            .await?;

        let posts = res.take::<Option<u64>>(0)?;

        Ok(posts.unwrap_or_default())
    }

    pub async fn get_by_followers(
        &self,
        user_id: &str,
        types: Vec<PostType>,
        pag: Pagination,
    ) -> CtxResult<Vec<PostView>> {
        let order_dir = pag.order_dir.unwrap_or(QryOrder::DESC).to_string();
        let fields = PostView::get_select_query_fields();

        let query = format!(
            "SELECT {fields} FROM {TABLE_NAME}
            WHERE record::id(belongs_to) IN $user_ids AND type IN $types AND (type IN $public_post_types OR $user IN <-{ACCESS_TABLE_NAME}.in)
            ORDER BY id {order_dir} LIMIT $limit START $start;"
        );

        let full_query = format!(
            "LET $user_ids = SELECT VALUE record::id(out) FROM $user->{FOLLOW_TABLE_NAME}; {query}"
        );
        let mut res = self
            .db
            .query(full_query)
            .bind(("limit", pag.count))
            .bind(("start", pag.start))
            .bind(("types", types))
            .bind(("public_post_types", vec![PostType::Public, PostType::Idea]))
            .bind(("user", RecordId::new(TABLE_COL_USER, user_id)))
            .await?;

        let posts = res.take::<Vec<PostView>>(res.num_statements() - 1)?;

        Ok(posts)
    }

    pub async fn get_by_tag(&self, tag: &str, pagination: Pagination) -> CtxResult<Vec<PostView>> {
        let order_dir = pagination.order_dir.unwrap_or(QryOrder::DESC).to_string();
        let order_by = pagination.order_by.unwrap_or("id".to_string()).to_string();
        let fields = PostView::get_fields();
        let query = format!(
            "SELECT *, out.{{{fields}}} AS entity FROM $tag->{TAG_REL_TABLE_NAME}
             WHERE out.type IN $public_types AND out.belongs_to.type = $disc_type
             ORDER BY out.{} {} LIMIT $limit START $start;",
            order_by, order_dir
        );
        let mut res = self
            .db
            .query(query)
            .bind(("tag", RecordId::new(TAG_TABLE_NAME, tag)))
            .bind(("public_types", vec![PostType::Public, PostType::Idea]))
            .bind(("limit", pagination.count))
            .bind(("start", pagination.start))
            .bind(("disc_type", DiscussionType::Public))
            .await?;

        let posts = res.take::<Vec<PostView>>((0, "entity"))?;
        Ok(posts)
    }

    pub async fn delete(&self, post_id: &str) -> AppResult<()> {
        let _ = self
            .db
            .query("BEGIN TRANSACTION; LET $reply_ids = (SELECT VALUE id FROM reply WHERE belongs_to = $post); DELETE reply WHERE belongs_to IN $reply_ids; DELETE reply WHERE belongs_to = $post; DELETE $post WHERE tasks_nr = 0; COMMIT TRANSACTION;")
            .bind(("post", RecordId::new(TABLE_NAME, post_id)))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?
            .check();
        Ok(())
    }

    pub async fn create(&self, data: CreatePost) -> CtxResult<PostView> {
        let mut res = self
            .db
            .query(format!(
                "CREATE {TABLE_NAME} CONTENT $data RETURN {}",
                PostView::get_select_query_fields()
            ))
            .bind(("data", data))
            .await
            .map_err(|e| CtxError::from(self.ctx)(e))?;
        let data = res.take::<Option<PostView>>(0)?;
        Ok(data.unwrap())
    }

    pub fn get_new_post_thing() -> RecordId {
        // id is ULID for sorting by time
        RecordId::new(TABLE_NAME, RecordIdKey::rand())
    }
}
