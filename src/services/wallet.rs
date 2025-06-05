use crate::{
    entities::user_auth::local_user_entity::LocalUserDbService,
    interfaces::payment::PaymentInterface,
    middleware::{
        ctx::Ctx,
        db,
        error::{AppError, AppResult},
        utils::db_utils::IdentIdName,
    },
};

pub struct SendMoneyTo {
    pub account: String,
    pub amount: f64,
    pub currency: String,
}

pub struct WalletService<'a> {
    user_repository: LocalUserDbService<'a>,
    payment: Box<dyn PaymentInterface + Send + Sync>,
}

impl<'a> WalletService<'a> {
    pub fn new(
        db: &'a db::Db,
        ctx: &'a Ctx,
        payment: Box<dyn PaymentInterface + Send + Sync>,
    ) -> Self {
        Self {
            user_repository: LocalUserDbService { db: db, ctx: ctx },
            payment,
        }
    }
    pub async fn withdraw(&self, account_id: &str) -> AppResult<()> {
        let account =
            self.payment
                .get_account(&account_id)
                .await
                .map_err(|e| AppError::Stripe {
                    source: "Account not found".to_string(),
                })?;

        // get data from draft transaction

        // lock user
        //
        // get stripe balance
        //
        // get darve finance account

        let result = self
            .payment
            .outbound_payments(&account.id, "eur", 100.00, None)
            .await;

        match result {
            Ok(_) => {
                // update transaction status pending
                Ok(())
            }
            Err(e) => {
                // inform user(transaction status/ notification)
                Err(AppError::Stripe {
                    source: format!("Send money error: {e:?}"),
                })
            }
        }
    }
}
