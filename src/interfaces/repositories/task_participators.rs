use async_trait::async_trait;

#[async_trait]
pub trait TaskParticipatorsRepositoryInterface {
    async fn create(
        &self,
        task_id: &str,
        user_id: &str,
        tx_id: &str,
        amount: u64,
        currency: &str,
    ) -> Result<String, String>;

    async fn update(
        &self,
        id: &str,
        tx_id: &str,
        amount: u64,
        currency: &str,
    ) -> Result<(), String>;
}
