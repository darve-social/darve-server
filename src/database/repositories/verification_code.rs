use std::sync::Arc;

use async_trait::async_trait;
use surrealdb::sql::Thing;

use crate::{
    database::client::Db,
    entities::verification_code::{VerificationCode, VerificationCodeFor},
    interfaces::repositories::verification_code::VerificationCodeRepositoryInterface,
    middleware::error::AppError,
};
pub const VERIFICATION_CODE_TABLE_NAME: &str = "verification_code";

#[derive(Debug)]
pub struct VerificationCodeRepository {
    client: Arc<Db>,
}

impl VerificationCodeRepository {
    pub fn new(client: Arc<Db>) -> Self {
        Self { client }
    }

    pub async fn mutate_db(&self) -> Result<(), AppError> {
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
impl VerificationCodeRepositoryInterface for VerificationCodeRepository {
    async fn get_by_user(
        &self,
        user_id: &str,
        use_for: VerificationCodeFor,
    ) -> Result<VerificationCode, String> {
        let id = Thing::try_from(user_id).map_err(|_| "User Id is invalid".to_string())?;

        let qry = format!(
            "SELECT * FROM {VERIFICATION_CODE_TABLE_NAME} WHERE user = $user_id AND use_for = $use_for;"
        );
        let mut res = self
            .client
            .query(qry)
            .bind(("user_id", id))
            .bind(("use_for", use_for))
            .await
            .map_err(|e| e.to_string())?;
        let data: Option<VerificationCode> = res.take(0).map_err(|e| e.to_string())?;
        match data {
            Some(v) => Ok(v),
            None => Err("Not Found".to_string()),
        }
    }
    async fn increase_attempt(&self, code_id: &str) -> Result<(), String> {
        let id = Thing::try_from(code_id).map_err(|_| "Id is invalid".to_string())?;
        let res = self
            .client
            .query("UPDATE $code_id SET failed_code_attempts += 1;")
            .bind(("code_id", id))
            .await
            .map_err(|e| e.to_string())?;
        res.check().map_err(|e| e.to_string())?;
        Ok(())
    }
    async fn delete(&self, code_id: &str) -> Result<(), String> {
        let id = Thing::try_from(code_id).map_err(|_| "Id is invalid".to_string())?;
        let _: Option<VerificationCode> = self
            .client
            .delete((id.tb, id.id.to_raw()))
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn create(
        &self,
        user_id: &str,
        code: &str,
        email: &str,
        use_for: VerificationCodeFor,
    ) -> Result<VerificationCode, String> {
        let user_thing = Thing::try_from(user_id).map_err(|_| "User Id is invalid".to_string())?;
        let qry = format!("
            BEGIN TRANSACTION;
                DELETE FROM {VERIFICATION_CODE_TABLE_NAME} WHERE user = $user_id AND use_for = $use_for;
                CREATE {VERIFICATION_CODE_TABLE_NAME} SET user=$user_id, code=$code, email=$email, use_for=$use_for;
            COMMIT TRANSACTION;
        ");
        let mut res = self
            .client
            .query(qry)
            .bind(("user_id", user_thing))
            .bind(("code", code.to_string()))
            .bind(("email", email.to_string()))
            .bind(("use_for", use_for))
            .await
            .map_err(|e| e.to_string())?;

        let data: VerificationCode = res
            .take::<Option<VerificationCode>>(1)
            .map_err(|e| e.to_string())?
            .unwrap();

        Ok(data)
    }
}
