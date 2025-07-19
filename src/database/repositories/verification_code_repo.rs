use crate::database::repository_impl::Repository;
use crate::database::repository_traits::RepositoryCore;
use crate::database::surrdb_utils::get_str_id_thing;
use crate::entities::community::discussion_entity::USER_TABLE_NAME;
use crate::{
    entities::verification_code::{VerificationCodeEntity, VerificationCodeFor},
    interfaces::repositories::verification_code_ifce::VerificationCodeRepositoryInterface,
    middleware::error::AppError,
};
use async_trait::async_trait;
use surrealdb::sql::Thing;

pub const VERIFICATION_CODE_TABLE_NAME: &str = "verification_code";

impl Repository<VerificationCodeEntity> {
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
        DEFINE TABLE IF NOT EXISTS {VERIFICATION_CODE_TABLE_NAME} SCHEMAFULL;
        DEFINE FIELD IF NOT EXISTS user ON TABLE {VERIFICATION_CODE_TABLE_NAME} TYPE record<local_user>;
        DEFINE FIELD IF NOT EXISTS email ON TABLE {VERIFICATION_CODE_TABLE_NAME} TYPE string;
        DEFINE FIELD IF NOT EXISTS use_for ON TABLE {VERIFICATION_CODE_TABLE_NAME} TYPE string;
        DEFINE FIELD IF NOT EXISTS code ON TABLE {VERIFICATION_CODE_TABLE_NAME} TYPE string;
        DEFINE FIELD IF NOT EXISTS failed_code_attempts ON TABLE {VERIFICATION_CODE_TABLE_NAME} TYPE number DEFAULT 0;
        DEFINE FIELD IF NOT EXISTS r_created ON TABLE {VERIFICATION_CODE_TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
        DEFINE INDEX IF NOT EXISTS user_use_for_idx ON TABLE {VERIFICATION_CODE_TABLE_NAME} COLUMNS user, use_for UNIQUE;
    ");
        let local_user_mutation = self.client.query(sql).await?;

        local_user_mutation
            .check()
            .expect("should mutate local_user");

        Ok(())
    }
}

#[async_trait]
impl VerificationCodeRepositoryInterface for Repository<VerificationCodeEntity> {
    async fn get_by_user(
        &self,
        user_id: &str,
        use_for: VerificationCodeFor,
    ) -> Result<VerificationCodeEntity, surrealdb::Error> {
        // let user_thing = Thing::try_from((USER_TABLE_NAME,user_id)).map_err(|_|surrealdb::error::Db::IdInvalid {value: user_id.to_string()})?;
        let user_thing = get_str_id_thing(USER_TABLE_NAME, user_id)?;

        let qry = format!(
            "SELECT * FROM {VERIFICATION_CODE_TABLE_NAME} WHERE user = $user_id AND use_for = $use_for;"
        );
        let mut res = self
            .client
            .query(qry)
            .bind(("user_id", user_thing.clone()))
            .bind(("use_for", use_for.clone()))
            .await?;

        let data: Option<VerificationCodeEntity> = res.take(0)?;
        match data {
            Some(v) => Ok(v),
            None => Err(surrealdb::Error::from(surrealdb::error::Db::IdNotFound {
                rid: format!("user_id={user_id} use_for={use_for:?}"),
            })),
        }
    }

    async fn increase_attempt(&self, code_id: &str) -> Result<(), surrealdb::Error> {
        let id = self.get_thing(code_id);
        let res = self
            .client
            .query("UPDATE $code_id SET failed_code_attempts += 1;")
            .bind(("code_id", id))
            .await?;
        res.check()?;
        Ok(())
    }

    async fn create(
        &self,
        user_id: &str,
        code: &str,
        email: &str,
        use_for: VerificationCodeFor,
    ) -> Result<VerificationCodeEntity, surrealdb::Error> {
        // let user_thing = Thing::try_from(user_id).map_err(|_| "User Id is invalid".to_string())?;
        // let user_thing = get_str_id_thing(USER_TABLE_NAME, user_id)?;
        let qry = format!("
            BEGIN TRANSACTION;
                DELETE FROM {VERIFICATION_CODE_TABLE_NAME} WHERE user = $user_id AND use_for = $use_for;
                CREATE {VERIFICATION_CODE_TABLE_NAME} SET user=$user_id, code=$code, email=$email, use_for=$use_for;
            COMMIT TRANSACTION;
        ");
        let mut res = self
            .client
            .query(qry)
            .bind(("user_id", Thing::from((USER_TABLE_NAME, user_id))))
            .bind(("code", code.to_string()))
            .bind(("email", email.to_string()))
            .bind(("use_for", use_for))
            .await?;

        let data: VerificationCodeEntity = res
            .take::<Option<VerificationCodeEntity>>(1)?
            .expect("record created");

        Ok(data)
    }

    async fn delete(&self, code_id: &str) -> Result<(), surrealdb::Error> {
        let id = self.get_thing(code_id);
        let _: Option<VerificationCodeEntity> = self.client.delete((id.tb, id.id.to_raw())).await?;
        Ok(())
    }
}
