use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use validator::Validate;

use crate::database::client::Db;
use crate::entities::community::discussion_entity::DiscussionDenyRule;
use crate::middleware;
use crate::middleware::utils::string_utils::get_str_thing;
use middleware::utils::db_utils::{get_entity, with_not_found_err, IdentIdName};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};

use super::discussion_entity::DiscussionDbService;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct Community {
    pub id: Thing,
    pub created_at: DateTime<Utc>,
    pub created_by: Thing,
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

    pub async fn create_profile(
        &self,
        user_id: Thing,
        disc_deny_rules: Option<Vec<DiscussionDenyRule>>,
    ) -> CtxResult<Community> {
        let community_id = CommunityDbService::get_profile_community_id(&user_id);
        let disc_id = DiscussionDbService::get_profile_discussion_id(&user_id);
        let idea_id = DiscussionDbService::get_idea_discussion_id(&user_id);

        let mut result = self
            .db
            .query("BEGIN TRANSACTION;")
            .query(
                "CREATE $idea SET belongs_to=$community, created_by=$user, deny_rules=$deny_rules;",
            )
            .query(
                "CREATE $disc SET belongs_to=$community, created_by=$user, deny_rules=$deny_rules;",
            )
            .query("RETURN CREATE $community SET created_by=$user;")
            .query("COMMIT TRANSACTION;")
            .bind(("user", user_id))
            .bind(("disc", disc_id.clone()))
            .bind(("idea", idea_id.clone()))
            .bind(("community", community_id.clone()))
            .bind(("deny_rules", disc_deny_rules))
            .await?;
        let comm = result.take::<Option<Community>>(0)?;
        Ok(comm.ok_or(AppError::EntityFailIdNotFound {
            ident: community_id.to_raw(),
        })?)
    }

    pub fn get_profile_community_id(user_id: &Thing) -> Thing {
        Thing::from((Self::get_table_name().to_string(), user_id.id.to_raw()))
    }
}
