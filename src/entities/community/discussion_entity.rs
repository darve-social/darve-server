use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

use middleware::utils::db_utils::{
    get_entity_view, with_not_found_err, IdentIdName, ViewFieldSelector,
};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};

use crate::database::client::Db;
use crate::database::surrdb_utils::record_id_key_to_string;
use crate::database::table_names::ACCESS_TABLE_NAME;
use crate::entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME;
use crate::middleware;
use crate::middleware::utils::db_utils::{Pagination, QryOrder};
use crate::middleware::utils::string_utils::get_str_thing;

use super::{community_entity, post_entity};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SurrealValue)]
pub enum DiscussionType {
    Private,
    Public,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct Discussion {
    pub id: RecordId,
    pub belongs_to: RecordId,
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_uri: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: RecordId,
    pub r#type: DiscussionType,
}

impl Discussion {
    pub fn is_profile(&self) -> bool {
        self.id == DiscussionDbService::get_profile_discussion_id(&self.created_by)
    }
}
#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreateDiscussionEntity {
    pub id: Option<RecordId>,
    pub belongs_to: RecordId,
    pub title: String,
    pub image_uri: Option<String>,
    pub created_by: RecordId,
    pub r#type: DiscussionType,
}

pub struct DiscussionDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "discussion";
pub const COMMUNITY_TABLE_NAME: &str = community_entity::TABLE_NAME;
pub const POST_TABLE_NAME: &str = post_entity::TABLE_NAME;

impl<'a> DiscussionDbService<'a> {
    pub fn get_table_name() -> &'static str {
        TABLE_NAME
    }
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS belongs_to ON TABLE {TABLE_NAME} TYPE record<{COMMUNITY_TABLE_NAME}>;
    DEFINE FIELD IF NOT EXISTS title ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS image_uri ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS type ON TABLE {TABLE_NAME} TYPE string;
    DEFINE FIELD IF NOT EXISTS created_by ON TABLE {TABLE_NAME} TYPE record<{USER_TABLE_NAME}>;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS updated_at ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE time::now();
    DEFINE INDEX IF NOT EXISTS idx_type ON TABLE {TABLE_NAME} COLUMNS type;
    DEFINE INDEX IF NOT EXISTS idx_title ON TABLE {TABLE_NAME} COLUMNS title;
");
        let mutation = self.db.query(sql).await?;
        mutation.check().expect("should mutate domain");

        Ok(())
    }

    pub async fn get_view_by_id<T: for<'b> Deserialize<'b> + SurrealValue + ViewFieldSelector>(
        &self,
        id: &str,
    ) -> CtxResult<T> {
        let thing = get_str_thing(id)?;
        let ident = IdentIdName::Id(thing);
        self.get_view(ident).await
    }

    pub async fn get_view<T: for<'b> Deserialize<'b> + SurrealValue + ViewFieldSelector>(
        &self,
        ident_id_name: IdentIdName,
    ) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }
    pub async fn get_by_id_with_user<T: for<'b> Deserialize<'b> + SurrealValue + ViewFieldSelector>(
        &self,
        id: &str,
        user_id: &str,
    ) -> CtxResult<T> {
        let fields = T::get_select_query_fields();
        let query = format!("SELECT {fields} FROM $disc");
        let mut res = self
            .db
            .query(query)
            .bind(("user", RecordId::new(USER_TABLE_NAME, user_id)))
            .bind(("disc", RecordId::new(TABLE_NAME, id)))
            .await?;
        let data = res.take::<Option<T>>(0)?;
        Ok(data.ok_or(AppError::EntityFailIdNotFound {
            ident: id.to_string(),
        })?)
    }

    pub async fn get_by_type<T: for<'b> Deserialize<'b> + SurrealValue + ViewFieldSelector>(
        &self,
        user_id: &str,
        disc_type: Option<DiscussionType>,
        pagination: Pagination,
    ) -> CtxResult<Vec<T>> {
        let order_dir = pagination.order_dir.unwrap_or(QryOrder::DESC).to_string();
        let order_by = pagination.order_by.unwrap_or("created_at".to_string());

        let query_by_type = match disc_type {
            Some(_) => "type=$disc_type AND ",
            None => "",
        };

        let fields = T::get_select_query_fields();

        let query = format!(
            "SELECT {fields} FROM {TABLE_NAME} WHERE {query_by_type} <-{ACCESS_TABLE_NAME}.in CONTAINS $user
                 ORDER BY {order_by} {order_dir} LIMIT $limit START $start;",
        );
        let mut res = self
            .db
            .query(query)
            .bind(("user", RecordId::new(USER_TABLE_NAME, user_id)))
            .bind(("disc_type", disc_type))
            .bind(("limit", pagination.count))
            .bind(("start", pagination.start))
            .await?;
        let data = res.take::<Vec<T>>(0)?;
        Ok(data)
    }

    pub async fn create(&self, data: CreateDiscussionEntity) -> CtxResult<Discussion> {
        let disc: Option<Discussion> = self
            .db
            .create(TABLE_NAME)
            .content(data)
            .await
            .map_err(CtxError::from(self.ctx))?;
        let disc = disc.unwrap();
        Ok(disc)
    }

    pub async fn update(&self, disc_id: &str, title: &str) -> CtxResult<Discussion> {
        let disc = self
            .db
            .query("UPDATE $disc SET title=$title")
            .bind(("disc", RecordId::new(TABLE_NAME, disc_id)))
            .bind(("title", title.to_string()))
            .await
            .map_err(CtxError::from(self.ctx))?
            .take::<Option<Discussion>>(0)?;

        Ok(disc.ok_or(AppError::EntityFailIdNotFound {
            ident: disc_id.to_string(),
        })?)
    }

    pub fn get_profile_discussion_id(user_id: &RecordId) -> RecordId {
        RecordId::new(TABLE_NAME, record_id_key_to_string(&user_id.key))
    }
}
