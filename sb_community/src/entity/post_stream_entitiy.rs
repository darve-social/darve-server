use serde::Deserialize;
use std::collections::HashMap;
use surrealdb::sql::Thing;

use crate::entity::post_entitiy::Post;
use sb_middleware::db;
use sb_middleware::error::AppResult;
use sb_middleware::utils::db_utils::QryBindingsVal;
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};
use sb_user_auth::entity::follow_entitiy::FollowDbService;

pub struct PostStreamDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "post_stream";
const TABLE_POST: &str = crate::entity::post_entitiy::TABLE_NAME;
const TABLE_USER: &str = sb_user_auth::entity::local_user_entity::TABLE_NAME;

impl<'a> PostStreamDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} TYPE RELATION IN {TABLE_USER} OUT {TABLE_POST} ENFORCED SCHEMAFULL PERMISSIONS NONE;
    DEFINE INDEX in_out_unique_idx ON {TABLE_NAME} FIELDS in, out UNIQUE;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    ");
        let mutation = self.db.query(sql).await?;

        &mutation.check().expect("should mutate reply");

        Ok(())
    }

    /*pub async fn add_to_stream(&self, user: Thing, post: Thing) -> CtxResult<bool> {
        let qry = format!("RELATE $in->{TABLE_NAME}->$out");
        let res = self
            .db
            .query(qry)
            .bind(("in", user))
            .bind(("out", post))
            .await?;
        res.check()?;
        Ok(true)
    }*/

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
        self.add_to_users_stream(notify_followers, post_id)
            .await
    }

    pub async fn user_posts_stream(&self, user: Thing) -> CtxResult<Vec<Thing>> {
        let qry = format!("SELECT ->{TABLE_NAME}->{TABLE_POST} as posts FROM <record>$user;");
        self.get_stream_posts_qry::<Thing>(qry, user).await
    }

    async fn get_stream_posts_qry<T: for<'de> Deserialize<'de>>(
        &self,
        qry: String,
        user_id: Thing,
    ) -> CtxResult<Vec<T>> {
        let mut res = self.db.query(qry).bind(("user", user_id.to_raw())).await?;

        let res: Option<Vec<T>> = res.take("posts")?;
        Ok(res.unwrap_or(vec![]))
    }
}
