use async_trait::async_trait;

use crate::{database::query_builder::SurrealQueryBuilder, entities::task_donor::TaskDonor};

#[async_trait]
pub trait TaskDonorsRepositoryInterface {
    fn build_create_query(
        &self,
        query: SurrealQueryBuilder,
        task_id: &str,
        user_id: &str,
        tx_id: &str,
        amount: u64,
        currency: &str,
    ) -> SurrealQueryBuilder;

    fn build_update_query(
        &self,
        query: SurrealQueryBuilder,
        id: &str,
        tx_id: &str,
        amount: u64,
        currency: &str,
    ) -> SurrealQueryBuilder;

    async fn create(
        &self,
        task_id: &str,
        user_id: &str,
        tx_id: &str,
        amount: u64,
        currency: &str,
    ) -> Result<TaskDonor, String>;

    async fn update(
        &self,
        id: &str,
        tx_id: &str,
        amount: u64,
        currency: &str,
    ) -> Result<TaskDonor, String>;
}
