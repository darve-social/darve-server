use async_trait::async_trait;

#[async_trait]
pub trait SendEmailInterface {
    async fn send(&self, emails: Vec<String>, body: &str, subject: &str) -> Result<(), String>;
}
