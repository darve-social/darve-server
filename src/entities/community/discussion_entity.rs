use serde::{Deserialize, Serialize};
use surrealdb::sql::{Id, Thing};
use validator::Validate;

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

use super::discussion_topic_entity::{self, DiscussionTopic};
use super::{community_entity, post_entity};

#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
pub struct Discussion {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    // belongs_to=community
    pub belongs_to: Thing,
    #[validate(length(min = 5, message = "Min 5 characters"))]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topics: Option<Vec<Thing>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_discussion_user_ids: Option<Vec<Thing>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_post_id: Option<Thing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    pub created_by: Thing,
    pub private_discussion_users_final: bool,
}

pub struct DiscussionDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "discussion";
pub const DISCUSSION_TOPIC_TABLE_NAME: &str = discussion_topic_entity::TABLE_NAME;
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
    DEFINE FIELD IF NOT EXISTS private_discussion_users_final ON TABLE {TABLE_NAME} TYPE bool DEFAULT false;
    DEFINE FIELD IF NOT EXISTS topics ON TABLE {TABLE_NAME} TYPE option<set<record<{DISCUSSION_TOPIC_TABLE_NAME}>, 25>>;
    DEFINE FIELD IF NOT EXISTS private_discussion_user_ids ON TABLE {TABLE_NAME} TYPE option<set<record<{USER_TABLE_NAME}>, 125>>;
        // ASSERT record::exists($value);
    DEFINE FIELD IF NOT EXISTS created_by ON TABLE {TABLE_NAME} TYPE record<{USER_TABLE_NAME}>;
    DEFINE FIELD IF NOT EXISTS latest_post_id ON TABLE {TABLE_NAME} TYPE option<record<{POST_TABLE_NAME}>>;
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    DEFINE INDEX IF NOT EXISTS idx_private_discussion_user_ids ON TABLE {TABLE_NAME} COLUMNS private_discussion_user_ids;
    DEFINE INDEX IF NOT EXISTS idx_title ON TABLE {TABLE_NAME} COLUMNS title;
    DEFINE INDEX IF NOT EXISTS idx_private_discussion_users_final ON TABLE {TABLE_NAME} COLUMNS private_discussion_users_final;
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

    pub async fn get_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        ident_id_name: IdentIdName,
    ) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn get_by_private_users(&self, user_ids: Vec<&str>) -> CtxResult<Discussion> {
        let user_things = user_ids.iter().fold(vec![], |mut res, id| {
            match Thing::try_from(*id) {
                Ok(v) => res.push(v),
                Err(_) => (),
            };
            res
        });

        let query = format!(
            "SELECT * FROM {TABLE_NAME} WHERE 
                private_discussion_users_final = true
                AND private_discussion_user_ids != NONE
                AND array::sort(private_discussion_user_ids) = array::sort($user_ids);",
        );

        let mut res = self.db.query(query).bind(("user_ids", user_things)).await?;

        let data = res.take::<Option<Discussion>>(0)?;
        match data {
            Some(v) => Ok(v),
            None => Err(AppError::EntityFailIdNotFound {
                ident: user_ids.join(",").to_string(),
            }
            .into()),
        }
    }

    pub async fn get_by_chat_room_user(&self, user_id: &str) -> CtxResult<Vec<Discussion>> {
        let user_thing = Thing::try_from(user_id).map_err(|_| AppError::Generic {
            description: "error parse into Thing".to_string(),
        })?;

        let query =
            format!("SELECT * FROM {TABLE_NAME} WHERE private_discussion_user_ids CONTAINS $user");
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

    pub async fn create_update(&self, mut record: Discussion) -> CtxResult<Discussion> {
        let resource = record
            .id
            .clone()
            .unwrap_or(Thing::from((TABLE_NAME.to_string(), Id::ulid())));

        record.r_created = None;
        let disc: Option<Discussion> = self
            .db
            .upsert((resource.tb, resource.id.to_raw()))
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))?;
        let disc = disc.unwrap();
        let auth = Authorization {
            authorize_record_id: disc.id.clone().unwrap(),
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

    pub async fn set_latest_post_id(
        &self,
        discussion_id: Thing,
        post_id: Thing,
    ) -> CtxResult<Discussion> {
        let q = "UPDATE $ident SET latest_post_id=$new_last_msg";

        let disc: Option<Discussion> = self
            .db
            .query(q)
            .bind(("ident", discussion_id.clone()))
            .bind(("new_last_msg", post_id))
            .await?
            .take(0)?;
        disc.ok_or(self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
            ident: discussion_id.to_raw(),
        }))
    }

    pub async fn add_topic(&self, discussion_id: Thing, topic_id: Thing) -> CtxResult<Discussion> {
        let q = "UPDATE $ident SET topics+=$new_topic";

        let disc: Option<Discussion> = self
            .db
            .query(q)
            .bind(("ident", discussion_id.clone()))
            .bind(("new_topic", topic_id))
            .await?
            .take(0)?;
        disc.ok_or(self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
            ident: discussion_id.to_raw(),
        }))
    }

    pub fn get_profile_discussion_id(user_id: &Thing) -> Thing {
        Thing::from((TABLE_NAME.to_string(), user_id.id.to_raw()))
    }

    pub async fn get_topics(&self, discussion_id: Thing) -> CtxResult<Vec<DiscussionTopic>> {
        let q = "SELECT topics.*.* FROM $discussion_id;";

        let disc: Option<Vec<DiscussionTopic>> = self
            .db
            .query(q)
            .bind(("discussion_id", discussion_id.clone()))
            .await?
            .take("topics")?;
        Ok(disc.unwrap_or(vec![]))
    }
}
