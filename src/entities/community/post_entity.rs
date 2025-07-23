use serde::{Deserialize, Serialize};
use surrealdb::err::Error::IndexExists;
use surrealdb::opt::PatchOp;
use surrealdb::sql::{Id, Thing};
use surrealdb::Error as ErrorSrl;
use validator::Validate;

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

use super::reply_entity::Reply;
use super::{discussion_entity, discussion_topic_entity};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discussion_topic: Option<Thing>,
    // #[serde(skip_serializing)]
    pub r_title_uri: Option<String>,
    pub content: Option<String>,
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
    pub tags: Option<Vec<String>>,
}

pub struct PostDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "post";
pub const TABLE_LIKE: &str = "like";

// origin
const TABLE_COL_DISCUSSION: &str = discussion_entity::TABLE_NAME;
const TABLE_COL_TOPIC: &str = discussion_topic_entity::TABLE_NAME;
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
    DEFINE FIELD IF NOT EXISTS r_title_uri ON TABLE {TABLE_NAME} VALUE string::slug($this.title);
    DEFINE INDEX IF NOT EXISTS {INDEX_BELONGS_TO_URI} ON TABLE {TABLE_NAME} COLUMNS belongs_to, r_title_uri UNIQUE;
    DEFINE FIELD IF NOT EXISTS {TABLE_COL_TOPIC} ON TABLE {TABLE_NAME} TYPE option<record<{TABLE_COL_TOPIC}>>
        ASSERT $value INSIDE (SELECT topics FROM ONLY $this.{TABLE_COL_BELONGS_TO}).topics;
    DEFINE INDEX IF NOT EXISTS {TABLE_COL_TOPIC}_idx ON TABLE {TABLE_NAME} COLUMNS {TABLE_COL_TOPIC};
    DEFINE FIELD IF NOT EXISTS content ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS media_links ON TABLE {TABLE_NAME} TYPE option<array<string>>;
    DEFINE FIELD IF NOT EXISTS metadata ON TABLE {TABLE_NAME} TYPE option<set<string>>;
    DEFINE FIELD IF NOT EXISTS replies_nr ON TABLE {TABLE_NAME} TYPE number DEFAULT 0;
    DEFINE FIELD IF NOT EXISTS likes_nr ON TABLE {TABLE_NAME} TYPE number DEFAULT 0;
    DEFINE FIELD IF NOT EXISTS tags ON TABLE {TABLE_NAME} TYPE option<array<string>>
        ASSERT {{ IF type::is::array($value) AND array::len($value) < 6 {{ RETURN true }}ELSE{{ IF type::is::none($value){{RETURN true}}ELSE{{THROW \"Maxi nr of tags is 5\"}} }} }};
    DEFINE INDEX IF NOT EXISTS tags_idx ON TABLE {TABLE_NAME} COLUMNS tags;
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();

    DEFINE TABLE IF NOT EXISTS {TABLE_LIKE} TYPE RELATION IN {TABLE_COL_USER} OUT {TABLE_NAME} ENFORCED SCHEMAFULL PERMISSIONS NONE;
    DEFINE INDEX IF NOT EXISTS in_out_unique_idx ON {TABLE_LIKE} FIELDS in, out UNIQUE;
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_LIKE} TYPE datetime DEFAULT time::now();
    DEFINE FIELD IF NOT EXISTS count ON TABLE {TABLE_LIKE} TYPE number;

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

    pub async fn get_by_tag(&self, tag: Option<String>, pag: Pagination) -> CtxResult<Vec<Post>> {
        let order_dir = pag.order_dir.unwrap_or(QryOrder::DESC).to_string();

        let query_str = format!(
            "SELECT * FROM {TABLE_NAME}
                WHERE {} {}
                ORDER BY id {order_dir} LIMIT $limit START $start;",
            self.get_access_query(),
            if tag.is_some() {
                "AND tags CONTAINS $tag"
            } else {
                ""
            }
        );

        let mut query = self.db.query(&query_str);

        query = query.bind(("limit", pag.count)).bind(("start", pag.start));

        if let Some(tag_value) = tag {
            query = query.bind(("tag", tag_value));
        }

        let posts = query.await?.take::<Vec<Post>>(0)?;

        Ok(posts)
    }

    fn get_access_query(&self) -> String {
        "record::id($this.belongs_to)=record::id($this.created_by)
        AND record::exists(type::record(string::concat('access_rule:',record::id($this.id))) )=false
        AND record::exists(type::record(string::concat('access_rule:',record::id($this.belongs_to))) )=false
        AND record::exists(type::record(string::concat('access_rule:',record::id($this.belongs_to.belongs_to))) )=false
        AND not($this.discussion_topic.*.access_rule)=true
        ".to_string()
    }

    fn create_filter(discussion_id: Thing, topic: Option<Thing>) -> IdentIdName {
        let filter_discussion = IdentIdName::ColumnIdent {
            column: TABLE_COL_BELONGS_TO.to_string(),
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

    pub async fn create_update(&self, record: Post) -> CtxResult<Post> {
        let resource = record.id.clone().unwrap_or(Self::get_new_post_thing());

        self.db
            .upsert((resource.tb, resource.id.to_raw()))
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

    // not used currently
    // pub async fn set_media_url(&self, post_id: &Thing, url: &str) -> CtxResult<Post> {
    //     // TODO add index para to change particular url
    //     let res: Option<Post> = self
    //         .db
    //         .update((post_id.tb.clone(), post_id.id.clone().to_string()))
    //         .patch(PatchOp::add("/media_links", [url]))
    //         .await
    //         .map_err(CtxError::from(self.ctx))?;
    //     res.ok_or_else(|| {
    //         self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
    //             ident: post_id.to_raw(),
    //         })
    //     })
    // }

    pub async fn like(&self, user: Thing, post: Thing, count: u8) -> CtxResult<u32> {
        let query = format!(
            "BEGIN TRANSACTION;
                RELATE $in->{TABLE_LIKE}->$out SET count=$count;
                LET $count = math::sum(SELECT VALUE <-{TABLE_LIKE}.count[0] ?? 0 FROM $out);
                UPDATE $out SET likes_nr=$count;
                RETURN $count;
            COMMIT TRANSACTION;"
        );
        let mut res = self
            .db
            .query(query)
            .bind(("in", user))
            .bind(("out", post))
            .bind(("count", count))
            .await?;

        let count = res.take::<Option<i64>>(0)?.unwrap() as u32;
        Ok(count)
    }

    pub async fn is_liked(&self, user: Thing, post: Thing) -> CtxResult<bool> {
        let query = format!("RETURN count($user->like[WHERE out == $post]) > 0;");
        let mut res = self
            .db
            .query(query)
            .bind(("user", user))
            .bind(("post", post))
            .await?;
        let is_liked = res.take::<Option<bool>>(0)?;
        Ok(is_liked.unwrap_or_default())
    }

    pub async fn unlike(&self, user: Thing, post: Thing) -> CtxResult<u32> {
        let query = format!(
            "BEGIN TRANSACTION;
                DELETE $in->{TABLE_LIKE} WHERE out=$out;
                LET $count = math::sum(SELECT VALUE <-{TABLE_LIKE}.count[0] ?? 0 FROM $out);
                UPDATE $out SET likes_nr=$count;
                RETURN $count;
            COMMIT TRANSACTION;"
        );
        let mut res = self
            .db
            .query(query)
            .bind(("in", user))
            .bind(("out", post))
            .await?;

        let count = res.take::<Option<i64>>(0)?.unwrap() as u32;
        Ok(count)
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
