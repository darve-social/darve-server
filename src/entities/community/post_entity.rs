use crate::database::table_names::{ACCESS_TABLE_NAME, TAG_REL_TABLE_NAME, TAG_TABLE_NAME};
use crate::entities::community::discussion_entity::DiscussionType;
use crate::middleware::utils::db_utils::ViewRelateField;
use chrono::{DateTime, Utc};
use middleware::utils::db_utils::{
    exists_entity, get_entity, get_entity_view, with_not_found_err, IdentIdName, Pagination,
    QryOrder, ViewFieldSelector,
};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use serde::{Deserialize, Serialize, Serializer};
use surrealdb::err::Error::IndexExists;
use surrealdb::opt::PatchOp;
use surrealdb::sql::{Id, Thing};
use surrealdb::Error as ErrorSrl;
use validator::Validate;

use crate::database::client::Db;
use crate::entities::community::discussion_entity::TABLE_NAME as DISC_TABLE_NAME;
use crate::entities::user_auth::follow_entity::TABLE_NAME as FOLLOW_TABLE_NAME;
use crate::entities::user_auth::local_user_entity;
use crate::middleware;
use crate::middleware::utils::string_utils::get_str_thing;
use crate::models::view::post::{LatestPostView, PostView};

use super::discussion_entity;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub enum PostType {
    Public,
    Private,
    Idea,
}

#[derive(Debug, PartialEq, Eq, Clone)]
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
    pub r#type: PostType,
}

#[derive(Debug, Serialize)]
pub struct CreatePost {
    pub id: Thing,
    pub belongs_to: Thing,
    pub created_by: Thing,
    pub title: String,
    pub content: Option<String>,
    pub media_links: Option<Vec<String>>,
    pub r#type: PostType,
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
    DEFINE FIELD IF NOT EXISTS type ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS updated_at ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE time::now();
    DEFINE INDEX IF NOT EXISTS idx_type ON TABLE {TABLE_NAME} COLUMNS type;
    DEFINE INDEX IF NOT EXISTS idx_title ON {TABLE_NAME} FIELDS title;
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

    pub async fn get_view_by_id<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        id: &str,
    ) -> CtxResult<T> {
        let thing = get_str_thing(id)?;
        let ident = IdentIdName::Id(thing);
        self.get_view(ident).await
    }

    pub async fn get_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        ident_id_name: IdentIdName,
    ) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn get_by_disc(
        &self,
        user_id: &str,
        disc_id: &str,
        filter_by_type: Option<PostType>,
        pag: Pagination,
    ) -> CtxResult<Vec<PostView>> {
        let order_dir = pag.order_dir.unwrap_or(QryOrder::DESC).to_string();
        let query_by_type = match filter_by_type {
            Some(_) => "AND type=$filter_by_type",
            None => "",
        };
        let fields = PostView::get_select_query_fields();

        let query = format!(
            "SELECT {fields} FROM {TABLE_NAME}
            WHERE belongs_to=$disc {query_by_type} AND (type IN $public_post_types OR $user IN <-{ACCESS_TABLE_NAME}.in)
            ORDER BY id {order_dir} LIMIT $limit START $start;"
        );

        let mut res = self
            .db
            .query(query)
            .bind(("limit", pag.count))
            .bind(("start", pag.start))
            .bind(("filter_by_type", filter_by_type))
            .bind(("public_post_types", vec![PostType::Public, PostType::Idea]))
            .bind(("disc", Thing::from((TABLE_COL_DISCUSSION, disc_id))))
            .bind(("user", Thing::from((TABLE_COL_USER, user_id))))
            .await?;

        let posts = res.take::<Vec<PostView>>(0)?;

        Ok(posts)
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

        let mut res = self
            .db
            .query(format!(
                "LET $user_ids = SELECT VALUE record::id(out) FROM $user->{FOLLOW_TABLE_NAME};"
            ))
            .query(query)
            .bind(("limit", pag.count))
            .bind(("start", pag.start))
            .bind(("types", types))
            .bind(("public_post_types", vec![PostType::Public, PostType::Idea]))
            .bind(("user", Thing::from((TABLE_COL_USER, user_id))))
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
            .bind(("tag", Thing::from((TAG_TABLE_NAME, tag))))
            .bind(("public_types", vec![PostType::Public, PostType::Idea]))
            .bind(("limit", pagination.count))
            .bind(("start", pagination.start))
            .bind(("disc_type", DiscussionType::Public))
            .await?;

        let posts = res.take::<Vec<PostView>>((0, "entity"))?;
        Ok(posts)
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

    pub async fn get_latest_posts(
        &self,
        user: Thing,
        search_test: Option<String>,
        disc_type: DiscussionType,
        pagination: Pagination,
    ) -> CtxResult<Vec<LatestPostView>> {
        let order_dir = pagination.order_dir.unwrap_or(QryOrder::DESC).to_string();
        let fields = LatestPostView::get_select_query_fields();
        let post_query = format!("SELECT {fields} FROM {TABLE_NAME}
            WHERE belongs_to=$parent.id AND (type IN $public_posts OR $user IN <-{ACCESS_TABLE_NAME}.in)
            ORDER BY id DESC LIMIT 1");

        let disc_query = format!(
            "SELECT VALUE ({post_query})[0] FROM {DISC_TABLE_NAME}
                WHERE type=$disc_type AND $user IN <-{ACCESS_TABLE_NAME}.in"
        );

        let search = match search_test {
            Some(_) => "WHERE $search_value IN title",
            None => "",
        };

        let query = format!("SELECT * FROM ({disc_query}) {search} ORDER BY id {order_dir} LIMIT $limit START $start;");

        let mut res = self
            .db
            .query(query)
            .bind(("user", user))
            .bind(("limit", pagination.count))
            .bind(("start", pagination.start))
            .bind(("public_posts", vec![PostType::Public, PostType::Idea]))
            .bind(("disc_type", disc_type))
            .bind(("$search_value", search_test))
            .bind(("disc_public", DiscussionType::Public))
            .await?;

        let data = res.take::<Vec<LatestPostView>>(0)?;
        Ok(data)
    }

    pub fn get_new_post_thing() -> Thing {
        // id is ULID for sorting by time
        Thing::from((TABLE_NAME.to_string(), Id::ulid()))
    }
}
