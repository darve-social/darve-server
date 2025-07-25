use serde::{Deserialize, Serialize};
use surrealdb::opt::PatchOp;
use surrealdb::sql::{Id, Thing};
use validator::Validate;

use crate::database::client::Db;
use crate::{entities::user_auth, middleware};
use middleware::utils::db_utils::{
    exists_entity, get_entity, get_entity_view, with_not_found_err, IdentIdName, ViewFieldSelector,
};
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};
use user_auth::access_right_entity::AccessRightDbService;
use user_auth::authorization_entity::{Authorization, AUTH_ACTIVITY_OWNER};

use super::discussion_entity::{self, Discussion, DiscussionDbService};

/// Community represents structure that holds discussions.
/// User has one profile community and can also create multiple custom communities.
/// User's profile discussion is generated from user id.
/// The main discussion in community is default_discussion.
/// Discussions can also be used as chat rooms or channels to add posts.

#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
pub struct Community {
    // random id or local_user_id for profile
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub name_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_discussion: Option<Thing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub courses: Option<Vec<Thing>>,
    pub created_by: Thing,
    pub stripe_connect_account_id: Option<String>,
    pub stripe_connect_complete: bool,
}

impl Community {
    pub fn new_user_community(user_id: &Thing) -> Self {
        Self {
            id: Some(CommunityDbService::get_profile_community_id(user_id)),
            title: None,
            name_uri: user_id.to_raw(),
            default_discussion: None,
            r_created: None,
            courses: None,
            created_by: user_id.clone(),
            stripe_connect_account_id: None,
            stripe_connect_complete: false,
        }
    }
}

pub struct CommunityDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "community";
pub const DISCUSSION_TABLE_NAME: &str = discussion_entity::TABLE_NAME;

impl<'a> CommunityDbService<'a> {
    pub fn get_table_name() -> &'static str {
        TABLE_NAME
    }
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD IF NOT EXISTS title ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS name_uri ON TABLE {TABLE_NAME} TYPE string VALUE string::slug(string::trim($value)) OR string::slug(string::trim($this.title))
         ASSERT string::len(string::slug(string::trim($value)))>4 OR string::len(string::slug(string::trim($this.title)))>4;
    DEFINE INDEX IF NOT EXISTS community_name_uri_idx ON TABLE {TABLE_NAME} COLUMNS name_uri UNIQUE;
    DEFINE FIELD IF NOT EXISTS default_discussion ON TABLE {TABLE_NAME} TYPE option<record<{DISCUSSION_TABLE_NAME}>>;
    // DEFINE FIELD IF NOT EXISTS following_posts_discussion ON TABLE {TABLE_NAME} TYPE option<record<{DISCUSSION_TABLE_NAME}>>;
    DEFINE FIELD IF NOT EXISTS courses ON TABLE {TABLE_NAME} TYPE option<array<record<course>>>;
    DEFINE FIELD IF NOT EXISTS created_by ON TABLE {TABLE_NAME} TYPE record<local_user>;
    DEFINE FIELD IF NOT EXISTS stripe_connect_account_id ON TABLE {TABLE_NAME} TYPE option<string>;
    DEFINE FIELD IF NOT EXISTS stripe_connect_complete ON TABLE {TABLE_NAME} TYPE bool DEFAULT false;
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD IF NOT EXISTS r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
");
        let mutation = self.db.query(sql).await?;
        mutation.check().expect("should mutate domain");

        Ok(())
    }

    pub async fn must_exist(&self, ident: IdentIdName) -> CtxResult<Thing> {
        let opt = exists_entity(self.db, TABLE_NAME.to_string(), &ident).await?;
        with_not_found_err(opt, self.ctx, ident.to_string().as_str())
    }

    pub async fn get(&self, ident_id_name: IdentIdName) -> CtxResult<Community> {
        let opt = get_entity::<Community>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn get_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        ident_id_name: IdentIdName,
    ) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), &ident_id_name).await?;
        with_not_found_err(opt, self.ctx, &ident_id_name.to_string().as_str())
    }

    pub async fn create_profile(&self, user_id: Thing) -> CtxResult<Community> {
        let community_id = CommunityDbService::get_profile_community_id(&user_id);
        let disc_id = DiscussionDbService::get_profile_discussion_id(&user_id);

        let query = format!(
            "BEGIN TRANSACTION;
                CREATE {disc_id} SET belongs_to = {community_id}, created_by = {user_id};
                RETURN CREATE {community_id} SET name_uri = \"{user_id}\", created_by = {user_id}, default_discussion = {disc_id};
            COMMIT TRANSACTION;"
        );
        let mut result = self.db.query(query).await?;
        let comm = result.take::<Option<Community>>(0)?;
        let comm = comm.unwrap();
        let auth1 = Authorization {
            authorize_record_id: community_id,
            authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
            authorize_height: 99,
        };

        let aright_db = AccessRightDbService {
            db: &self.db,
            ctx: &self.ctx,
        };

        aright_db
            .authorize(comm.created_by.clone(), auth1, None)
            .await?;

        Ok(comm)
    }

    pub async fn create_update(&self, mut record: Community) -> CtxResult<Community> {
        let resource = record
            .id
            .clone()
            .unwrap_or(Thing::from((TABLE_NAME.to_string(), Id::ulid())));
        record.r_created = None;
        // TODO in transaction

        let comm: Option<Community> = self
            .db
            .upsert((resource.tb, resource.id.to_raw()))
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))?;
        let mut comm = comm.unwrap();

        let comm_id = comm.id.clone().unwrap();
        if comm.default_discussion.is_none() {
            let disc = self
                .db
                .create(DISCUSSION_TABLE_NAME)
                .content(Discussion {
                    id: None,
                    title: None,
                    image_uri: None,
                    topics: None,
                    private_discussion_user_ids: None,
                    latest_post_id: None,
                    r_created: None,
                    belongs_to: comm_id.clone(),
                    created_by: comm.created_by.clone(),
                    private_discussion_users_final: false,
                })
                .await
                .map_err(CtxError::from(self.ctx))
                .map(|v: Option<Discussion>| v.unwrap())?;

            let comm_upd: CtxResult<Option<Community>> = self
                .db
                .update((&comm_id.tb, comm_id.id.clone().to_raw()))
                .patch(PatchOp::replace("/default_discussion", disc.id.clone()))
                .await
                .map_err(CtxError::from(self.ctx));
            comm = comm_upd?.ok_or(self.ctx.to_ctx_error(AppError::EntityFailIdNotFound {
                ident: comm_id.to_raw(),
            }))?;
        }

        let auth1 = Authorization {
            authorize_record_id: comm_id,
            authorize_activity: AUTH_ACTIVITY_OWNER.to_string(),
            authorize_height: 99,
        };
        let aright_db = AccessRightDbService {
            db: &self.db,
            ctx: &self.ctx,
        };
        aright_db
            .authorize(comm.created_by.clone(), auth1, None)
            .await?;

        Ok(comm)
    }

    pub async fn get_profile_community(&self, user_id: Thing) -> CtxResult<Community> {
        let user_comm_id = Self::get_profile_community_id(&user_id);
        let user_comm = self.get(IdentIdName::Id(user_comm_id.clone())).await;
        match user_comm {
            Ok(_) => user_comm,
            Err(err) => {
                if let AppError::EntityFailIdNotFound { .. } = err.error {
                    self.create_profile(user_id).await
                } else {
                    Err(self.ctx.to_ctx_error(err.error))
                }
            }
        }
    }

    pub fn get_profile_community_id(user_id: &Thing) -> Thing {
        Thing::from((Self::get_table_name().to_string(), user_id.id.to_raw()))
    }
}
