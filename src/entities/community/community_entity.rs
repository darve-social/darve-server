use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};
use validator::Validate;

use crate::database::client::Db;
use crate::database::surrdb_utils::record_id_key_to_string;
use crate::entities::community::discussion_entity::DiscussionType;
use crate::middleware;
use crate::middleware::utils::string_utils::get_str_thing;
use middleware::utils::db_utils::{get_entity, with_not_found_err, IdentIdName};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};

#[derive(Debug, Serialize, Deserialize, Validate, SurrealValue)]
pub struct Community {
    pub id: RecordId,
    pub created_at: DateTime<Utc>,
    pub created_by: RecordId,
}

pub struct CommunityDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "community";

impl<'a> CommunityDbService<'a> {
    pub fn get_table_name() -> &'static str {
        TABLE_NAME
    }
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS created_by ON TABLE {TABLE_NAME} TYPE record<local_user>;
    DEFINE FIELD IF NOT EXISTS created_at ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
");
        let mutation = self.db.query(sql).await?;
        mutation.check().expect("should mutate domain");

        Ok(())
    }

    pub async fn get(&self, ident_id_name: IdentIdName) -> CtxResult<Community> {
        let opt = get_entity::<Community>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn get_by_id(&self, id: &str) -> CtxResult<Community> {
        let ident = get_str_thing(id)?;
        let ident_id_name = IdentIdName::Id(ident);
        let opt = get_entity::<Community>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn create_profile(&self, disc_id: RecordId, user_id: RecordId) -> CtxResult<Community> {
        let community_id = CommunityDbService::get_profile_community_id(&user_id);

        let community_id_str = format!(
            "{}:{}",
            community_id.table.as_str(),
            record_id_key_to_string(&community_id.key)
        );
        let mut result = self
            .db
            .query("BEGIN TRANSACTION; CREATE $disc SET belongs_to=$community, created_by=$user, type=$type; RETURN CREATE $community SET created_by=$user; COMMIT TRANSACTION;")
            .bind(("user", user_id))
            .bind(("disc", disc_id))
            .bind(("type", DiscussionType::Public))
            .bind(("community", community_id.clone()))
            .await?;
        let comm = result.take::<Option<Community>>(2)?;
        Ok(comm.ok_or(AppError::EntityFailIdNotFound {
            ident: community_id_str,
        })?)
    }

    pub fn get_profile_community_id(user_id: &RecordId) -> RecordId {
        RecordId::new(
            Self::get_table_name(),
            record_id_key_to_string(&user_id.key),
        )
    }
}
