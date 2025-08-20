use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use middleware::utils::db_utils::{
    exists_entity, get_entity, get_entity_view, with_not_found_err, IdentIdName, ViewFieldSelector,
};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use user_auth::access_right_entity::AccessRightDbService;
use user_auth::authorization_entity::{Authorization, AUTH_ACTIVITY_OWNER};

use crate::database::client::Db;
use crate::entities::user_auth::{self, local_user_entity};
use crate::middleware;
use crate::middleware::utils::string_utils::get_str_thing;

use super::{community_entity, post_entity};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiscussionType {
    Private,
    Fixed,
    Public,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Discussion {
    pub id: Thing,
    pub belongs_to: Thing,
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_uri: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Thing,
    pub r#type: DiscussionType,
}

impl Discussion {
    pub fn is_profile(&self) -> bool {
        self.id == DiscussionDbService::get_profile_discussion_id(&self.created_by)
            || self.id == DiscussionDbService::get_idea_discussion_id(&self.created_by)
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateDiscussionEntity {
    pub id: Option<Thing>,
    pub belongs_to: Thing,
    pub title: String,
    pub image_uri: Option<String>,
    pub created_by: Thing,
    pub r#type: DiscussionType,
}

pub struct DiscussionDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "discussion";
pub const COMMUNITY_TABLE_NAME: &str = community_entity::TABLE_NAME;
pub const POST_TABLE_NAME: &str = post_entity::TABLE_NAME;
pub const USER_TABLE_NAME: &str = local_user_entity::TABLE_NAME;

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

    pub async fn must_exist(&self, ident: IdentIdName) -> CtxResult<Thing> {
        let opt = exists_entity(self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, ident.to_string().as_str())
    }

    pub async fn get(&self, ident_id_name: IdentIdName) -> CtxResult<Discussion> {
        let opt = get_entity::<Discussion>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn get_by_id(&self, id: &str) -> CtxResult<Discussion> {
        let thing = get_str_thing(id)?;
        let ident = IdentIdName::Id(thing);
        self.get(ident).await
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

    pub async fn get_by_chat_room_user(&self, user_id: &str) -> CtxResult<Vec<Discussion>> {
        let user_thing = Thing::try_from(user_id).map_err(|_| AppError::Generic {
            description: "error parse into Thing".to_string(),
        })?;

        let query = format!(
            "SELECT * FROM {TABLE_NAME} WHERE type != 'Public' AND <-has_access.in CONTAINS $user; "
        );
        let mut res = self.db.query(query).bind(("user", user_thing)).await?;
        let data: Vec<Discussion> = res.take::<Vec<Discussion>>(0)?;
        Ok(data)
    }

    pub async fn delete(&self, id: &str) -> CtxResult<()> {
        let thing = Thing::try_from(id).map_err(|_| AppError::Generic {
            description: "error into Thing".to_string(),
        })?;
        let _ = self
            .db
            .delete::<Option<Discussion>>((thing.tb, thing.id.to_raw()))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            });

        Ok(())
    }

    pub async fn create(&self, data: CreateDiscussionEntity) -> CtxResult<Discussion> {
        let disc: Option<Discussion> = self
            .db
            .create(TABLE_NAME)
            .content(data)
            .await
            .map_err(CtxError::from(self.ctx))?;
        let disc = disc.unwrap();
        let auth = Authorization {
            authorize_record_id: disc.id.clone(),
            authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
            authorize_height: 99,
        };
        let aright_db = AccessRightDbService {
            db: &self.db,
            ctx: &self.ctx,
        };
        // TODO in transaction
        aright_db
            .authorize(disc.created_by.clone(), auth, None)
            .await?;
        Ok(disc)
    }

    pub async fn update(&self, disc_id: &str, title: &str) -> CtxResult<Discussion> {
        let disc = self
            .db
            .query("UPDATE $disc SET title=$title")
            .bind(("disc", Thing::from((TABLE_NAME, disc_id))))
            .bind(("title", title.to_string()))
            .await
            .map_err(CtxError::from(self.ctx))?
            .take::<Option<Discussion>>(0)?;

        Ok(disc.ok_or(AppError::EntityFailIdNotFound {
            ident: disc_id.to_string(),
        })?)
    }

    pub async fn update_users(
        &self,
        disc_id: &str,
        users: Option<Vec<Thing>>,
    ) -> CtxResult<Discussion> {
        let disc = self
            .db
            .query("UPDATE $disc SET private_discussion_user_ids=$users")
            .bind(("disc", Thing::from((TABLE_NAME, disc_id))))
            .bind(("users", users))
            .await
            .map_err(CtxError::from(self.ctx))?
            .take::<Option<Discussion>>(0)?;

        Ok(disc.ok_or(AppError::EntityFailIdNotFound {
            ident: disc_id.to_string(),
        })?)
    }

    pub fn get_profile_discussion_id(user_id: &Thing) -> Thing {
        Thing::from((TABLE_NAME.to_string(), format!("{}_p", user_id.id.to_raw())))
    }

    pub fn get_idea_discussion_id(user_id: &Thing) -> Thing {
        Thing::from((TABLE_NAME.to_string(), format!("{}_i", user_id.id.to_raw())))
    }
}
