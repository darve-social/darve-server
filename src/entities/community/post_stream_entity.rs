use crate::database::client::Db;
use crate::middleware::utils::db_utils::ViewRelateField;
use serde::Deserialize;
use std::collections::HashMap;
use surrealdb::sql::Thing;

use middleware::error::AppResult;
use middleware::utils::db_utils::QryBindingsVal;
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};
use user_auth::follow_entity::FollowDbService;

use crate::entities::user_auth::{self, local_user_entity};
use crate::middleware;

use super::post_entity;

pub struct PostStreamDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "post_stream";
const TABLE_POST: &str = post_entity::TABLE_NAME;
const TABLE_USER: &str = local_user_entity::TABLE_NAME;

impl<'a> PostStreamDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} TYPE RELATION IN {TABLE_USER} OUT {TABLE_POST} ENFORCED SCHEMAFULL PERMISSIONS NONE;
    DEFINE INDEX IF NOT EXISTS in_out_unique_idx ON {TABLE_NAME} FIELDS in, out UNIQUE;
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    ");
        let mutation = self.db.query(sql).await?;

        mutation.check().expect("should mutate reply");

        Ok(())
    }

    pub async fn add_to_users_stream(&self, user_ids: Vec<Thing>, post: &Thing) -> AppResult<()> {
        let qry: Vec<QryBindingsVal<String>> = user_ids
            .into_iter()
            .enumerate()
            .map(|i_uid| self.create_qry(post, &i_uid.1, i_uid.0).ok())
            .filter(|v| v.is_some())
            .map(|v| v.unwrap())
            .collect();
        let qrys_bindings =
            qry.into_iter()
                .fold((vec![], HashMap::new()), |mut qrys_bindings, qbv| {
                    qrys_bindings.0.push(qbv.get_query_string());
                    qrys_bindings.1.extend(qbv.get_bindings());
                    qrys_bindings
                });
        let qry = self.db.query(qrys_bindings.0.join(""));
        let qry = qrys_bindings
            .1
            .into_iter()
            .fold(qry, |qry, n_val| qry.bind(n_val));
        let res = qry.await?;
        res.check()?;
        Ok(())
    }

    pub fn create_qry(
        &self,
        post_id: &Thing,
        user_id: &Thing,
        qry_ident: usize,
    ) -> AppResult<QryBindingsVal<String>> {
        let mut bindings: HashMap<String, String> = HashMap::new();
        bindings.insert(format!("in_{qry_ident}"), user_id.to_raw());
        bindings.insert(format!("out_{qry_ident}"), post_id.to_raw());
        let qry = format!(
            "RELATE (type::record($in_{qry_ident}))->{TABLE_NAME}->(type::record($out_{qry_ident}));"
        );
        Ok(QryBindingsVal::new(qry, bindings))
    }

    pub async fn to_user_follower_streams(&self, user_id: Thing, post_id: &Thing) -> AppResult<()> {
        let notify_followers: Vec<Thing> = FollowDbService {
            db: self.db,
            ctx: self.ctx,
        }
        .user_follower_ids(user_id.clone())
        .await?;
        self.add_to_users_stream(notify_followers, post_id).await
    }

    pub async fn get_posts<T: for<'b> Deserialize<'b> + ViewRelateField>(
        &self,
        user_id: Thing,
    ) -> CtxResult<Vec<T>> {
        let fields = T::get_fields();
        let qry = format!(
            "SELECT ->{TABLE_NAME}->{TABLE_POST}.{{{fields}}} as posts FROM <record>$user;"
        );
        let mut res = self
            .db
            .query(qry)
            .bind(("user", user_id))
            .await
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            })?;

        let posts = res
            .take::<Option<Vec<T>>>("posts")
            .map_err(|e| AppError::SurrealDb {
                source: e.to_string(),
            });

        Ok(posts?.unwrap_or_default())
    }
}
