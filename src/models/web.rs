use askama::Template;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/default-content.html")]
pub struct TaskRequestDonorView {
    pub id: Option<Thing>,
    pub user: Option<UserView>,
    pub amount: i64,
}

#[derive(Template, Serialize, Deserialize, Debug, Clone)]
#[template(path = "nera2/default-content.html")]
pub struct UserView {
    pub id: Thing,
    pub username: String,
    pub full_name: Option<String>,
}
