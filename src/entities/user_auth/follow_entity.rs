use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use middleware::db;
use middleware::{
    ctx::Ctx,
    error::{AppError, CtxResult},
};

use crate::middleware;

use super::local_user_entity::{self, LocalUser};

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
const TABLE_USER: &str = local_user_entity::TABLE_NAME;

impl<'a> FollowDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE IF NOT EXISTS {TABLE_NAME} TYPE RELATION IN {TABLE_USER} OUT {TABLE_USER} ENFORCED SCHEMAFULL PERMISSIONS NONE;
    DEFINE INDEX IF NOT EXISTS in_out_unique_idx ON {TABLE_NAME} FIELDS in, out UNIQUE;
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
");

        let mutation = self.db.query(sql).await?;
        mutation.check().expect("should mutate domain");

        Ok(())
    }

    pub async fn is_following(&self, user: Thing, follows: Thing) -> CtxResult<bool> {
        let qry = format!("SELECT count() FROM ONLY {TABLE_NAME} where in=<record>$in AND out=<record>$out LIMIT 1;");
        let mut res = self
            .db
            .query(qry)
            .bind(("in", user))
            .bind(("out", follows))
            .await?;
        let res: Option<i64> = res.take("count")?;
        Ok(res.unwrap_or(0) > 0)
    }

    pub async fn create_follow(&self, user: Thing, follows: Thing) -> CtxResult<bool> {
        let qry = format!("RELATE $in->{TABLE_NAME}->$out");
        let res = self
            .db
            .query(qry)
            .bind(("in", user))
            .bind(("out", follows))
            .await?;
        res.check()?;
        Ok(true)
    }

    pub async fn remove_follow(&self, user: Thing, unfollow: Thing) -> CtxResult<bool> {
        let qry = format!("DELETE $in->{TABLE_NAME} WHERE out=$out");
        self.db
            .query(qry)
            .bind(("in", user))
            .bind(("out", unfollow))
            .await?;
        Ok(true)
    }

    pub async fn user_follower_ids(&self, user: Thing) -> CtxResult<Vec<Thing>> {
        let qry = format!("SELECT <-{TABLE_NAME}<-{TABLE_USER} as followers FROM <record>$user;");
        self.get_followers_qry::<Thing>(qry, user).await
    }

    pub async fn user_followers_number(&self, user: Thing) -> CtxResult<i64> {
        let qry = format!("SELECT count(<-{TABLE_NAME}<-{TABLE_USER}) as nr FROM <record>$user;");
        self.get_nr_qry(qry, user).await
    }

    pub async fn user_followers(&self, user: Thing) -> CtxResult<Vec<LocalUser>> {
        let qry = format!("SELECT <-{TABLE_NAME}<-{TABLE_USER}.* as followers FROM <record>$user;");
        self.get_followers_qry::<LocalUser>(qry, user).await
    }

    async fn get_followers_qry<T: for<'de> Deserialize<'de>>(
        &self,
        qry: String,
        user_id: Thing,
    ) -> CtxResult<Vec<T>> {
        let mut res = self.db.query(qry).bind(("user", user_id.to_raw())).await?;
        let res: Option<Vec<T>> = res.take("followers")?;
        Ok(res.unwrap_or(vec![]))
    }

    async fn get_nr_qry(&self, qry: String, user_id: Thing) -> CtxResult<i64> {
        let mut res = self.db.query(qry).bind(("user", user_id.to_raw())).await?;
        let res: Option<i64> = res.take("nr")?;
        Ok(res.unwrap_or(0))
    }

    pub async fn user_following_number(&self, user: Thing) -> CtxResult<i64> {
        let qry = format!("SELECT count(->{TABLE_NAME}->{TABLE_USER}) as nr FROM <record>$user;");
        self.get_nr_qry(qry, user).await
    }

    pub async fn user_following(&self, user: Thing) -> CtxResult<Vec<LocalUser>> {
        let qry = format!("SELECT ->{TABLE_NAME}->{TABLE_USER}.* as following FROM <record>$user;");
        let mut res = self.db.query(qry).bind(("user", user.to_raw())).await?;
        let res: Option<Vec<LocalUser>> = res.take("following")?;
        Ok(res.unwrap_or(vec![]))
    }
}
