use std::fmt::Display;

use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::entity::local_user_entity::LocalUser;
use sb_middleware::db;
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Follow {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub r#in: Thing,
    pub out: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_created: Option<String>,
}


pub struct FollowDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "follow";
const TABLE_USER: &str = crate::entity::local_user_entity::TABLE_NAME;

impl<'a> FollowDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} TYPE RELATION FROM {TABLE_USER} TO {TABLE_USER} SCHEMAFULL PERMISSIONS none;
    DEFINE INDEX follow_idx ON TABLE {TABLE_NAME} COLUMNS in, out UNIQUE;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
");

        let mutation = self.db
            .query(sql)
            .await?;
        &mutation.check().expect("should mutate domain");

        Ok(())
    }

    pub async fn create_follow(&self, user: Thing, follows: Thing) -> CtxResult<bool> {
        let qry = format!("RELATE $in->{TABLE_NAME}->$out");
        self.db.query(qry)
            .bind(("in", user))
            .bind(("out", follows)).await?;
        Ok(true)
    }

    pub async fn remove_follow(&self, user: Thing, unfollow: Thing) -> CtxResult<bool> {
        let qry = format!("DELETE $in->{TABLE_NAME} WHERE out=$out");
        self.db.query(qry)
            .bind(("in", user))
            .bind(("out", unfollow)).await?;
        Ok(true)
    }

    pub async fn user_followers(&self, user: Thing) -> CtxResult<Vec<LocalUser>> {
        let qry = format!("SELECT <-{TABLE_NAME}<-{TABLE_USER}.* as followers FROM <record>$user;");
        let mut res = self.db.query(qry).bind(("user", user.to_raw())).await?;
        let res: Option<Vec<LocalUser>> = res.take("followers")?;
        Ok(res.unwrap_or(vec![]))
    }

    pub async fn user_following(&self, user: Thing) -> CtxResult<Vec<LocalUser>> {
        let qry = format!("SELECT ->{TABLE_NAME}->{TABLE_USER}.* as following FROM <record>$user;");
        let mut res = self.db.query(qry).bind(("user", user.to_raw())).await?;
        let res: Option<Vec<LocalUser>> = res.take("following")?;
        Ok(res.unwrap_or(vec![]))
    }
}

