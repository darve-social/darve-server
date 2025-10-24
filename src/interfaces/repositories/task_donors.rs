use async_trait::async_trait;
use surrealdb::{engine::any, method::Query};

use crate::entities::task_donor::TaskDonor;

#[async_trait]
pub trait TaskDonorsRepositoryInterface {
    fn build_create_query<'b>(
        &self,
        query: Query<'b, any::Any>,
        task_id: &str,
        user_id: &str,
        tx_id: &str,
        amount: u64,
        currency: &str,
    ) -> Query<'b, any::Any>;

    fn build_update_query<'b>(
        &self,
        query: Query<'b, any::Any>,
        id: &str,
        tx_id: &str,
        amount: u64,
        currency: &str,
    ) -> Query<'b, any::Any>;

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
