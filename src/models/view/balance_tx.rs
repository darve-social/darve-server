use crate::entities::task::task_request_entity::{TaskRequest, TABLE_NAME as TASK_TABLE_NAME};
use crate::entities::user_auth::local_user_entity::TABLE_NAME as USER_TABLE_NAME;
use crate::models::view::user::UserView;
use crate::{
    database::repositories::wallet::TransactionType,
    entities::wallet::wallet_entity::CurrencySymbol,
    middleware::utils::db_utils::ViewFieldSelector,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Deserialize, Debug, Serialize)]
pub struct CurrencyTransactionView {
    pub id: Thing,
    pub wallet: WalletView,
    pub with_wallet: WalletView,
    pub balance: i64,
    pub currency: CurrencySymbol,
    pub amount_in: Option<i64>,
    pub amount_out: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub description: Option<String>,
    pub r#type: Option<TransactionType>,
}

impl ViewFieldSelector for CurrencyTransactionView {
    fn get_select_query_fields() -> String {
        format!(
            "id,
            wallet.{{ 
                id, 
                user: type::thing('{USER_TABLE_NAME}', record::id(id)).* , 
                task: type::thing('{TASK_TABLE_NAME}', record::id(id)).* 
            }},
            with_wallet.{{ 
                id, 
                user: type::thing('{USER_TABLE_NAME}', record::id(id)).* , 
                task: type::thing('{TASK_TABLE_NAME}', record::id(id)).* 
            }},
        balance,
        amount_in,
        amount_out,
        currency,
        description,
        fee,
        type,
        created_at"
        )
    }
}
#[derive(Debug, Deserialize, Serialize)]
pub struct WalletView {
    pub id: Thing,
    pub user: Option<UserView>,
    pub task: Option<TaskRequest>,
}
