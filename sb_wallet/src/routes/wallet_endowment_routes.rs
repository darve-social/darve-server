use std::str::FromStr;

use crate::entity::funding_transaction_entity::FundingTransactionDbService;
use crate::entity::wallet_entitiy::{CurrencySymbol, WalletDbService};
use askama_axum::axum_core::response::IntoResponse;
use axum::body::Body;
use axum::extract::{FromRequest, Path, Request, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{async_trait, Router};
use stripe::{AccountId, Client, CreatePaymentIntent, Currency, Event, ProductId};
use surrealdb::sql::Thing;

use sb_middleware::ctx::Ctx;
use sb_middleware::error::{AppError, CtxResult};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::string_utils::get_string_thing;

const PRICE_USER_ID_KEY: &str = "user_id";
const PRODUCT_ID_KEY: &str = "product_id";

pub fn routes(state: CtxState) -> Router {
    let routes = Router::new()
        .route(
            "/api/user/wallet/endowment/:amount",
            get(request_endowment_intent),
        )
        .route("/api/stripe/endowment/webhook", post(handle_webhook));

    let routes = if state.is_development {
        routes.route(
            "/test/api/endow/:endow_user_id/:amount",
            get(test_endowment_transaction),
        )
    } else {
        routes
    };

    routes.with_state(state)
}

struct EndowmentIdent {
    user_id: Thing,
    amount: i64,
    pub action: String,
}

impl From<EndowmentIdent> for ProductId {
    fn from(value: EndowmentIdent) -> Self {
        let stripe_prod_id = format!(
            "{}~{}~{}",
            value.user_id.to_raw().replace(":", "-"),
            value.amount,
            value.action
        );
        ProductId::from_str(stripe_prod_id.as_str()).unwrap()
    }
}

struct MyStripeProductId(ProductId);

impl TryFrom<MyStripeProductId> for EndowmentIdent {
    type Error = AppError;

    fn try_from(value: MyStripeProductId) -> Result<Self, Self::Error> {
        let mut spl = value.0.as_str().split("~");
        let user_ident = spl.next().ok_or(AppError::Generic {
            description: "missing user_id part".to_string(),
        })?;
        let amount = spl.next().ok_or(AppError::Generic {
            description: "missing amount part".to_string(),
        })?;
        let amount = amount.parse::<i64>().map_err(|e| AppError::Generic {
            description: e.to_string(),
        })?;
        let action = spl
            .next()
            .ok_or(AppError::Generic {
                description: "missing action part".to_string(),
            })?
            .to_string();
        let user_id = get_string_thing(user_ident.replace("-", ":"))?;
        Ok(EndowmentIdent {
            user_id,
            amount,
            action,
        })
    }
}

async fn test_endowment_transaction(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path((endow_user_id, amount)): Path<(String, i64)>,
) -> CtxResult<Response> {
    println!("->> {:<12} - request endowment_transaction ", "HANDLER");

    if !ctx_state.is_development {
        return Err(AppError::AuthorizationFail {
            required: "Endpoint only available in development mode".to_string(),
        }
        .into());
    }

    let another_user_thing = get_string_thing(endow_user_id).expect("got thing");

    print!("another_user_id");

    let fund_service = FundingTransactionDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };
    let wallet_service = WalletDbService {
        db: &ctx_state._db,
        ctx: &ctx,
    };

    fund_service
        .user_endowment_tx(
            &another_user_thing,
            "ext_acc123".to_string(),
            "ext_tx_id_123".to_string(),
            amount,
            CurrencySymbol::USD,
        )
        .await
        .expect("created");

    let user1_bal = wallet_service
        .get_user_balance(&another_user_thing)
        .await
        .expect("got balance");

    Ok((StatusCode::OK, user1_bal.balance_usd.to_string()).into_response())
}

async fn request_endowment_intent(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(amount): Path<u32>,
) -> CtxResult<Response> {
    println!("->> {:<12} - request endowment payment ", "HANDLER");

    let user_id = ctx.user_id()?;
    println!("User ID retrieved: {:?}", user_id);

    let acc_id = AccountId::from_str(ctx_state.stripe_platform_account.as_str()).map_err(|e1| {
        ctx.to_ctx_error(AppError::Stripe {
            source: e1.to_string(),
        })
    })?;
    let client = Client::new(ctx_state.stripe_key).with_stripe_account(acc_id.clone());

    let amt = (amount * 100) as i64;

    let product_id: ProductId = {
        let product_title = "wallet_endowment".to_string();
        EndowmentIdent {
            user_id: get_string_thing(user_id.clone())?,
            amount: amt,
            action: product_title.clone(),
        }
        .into()
    };

    let create_pi = CreatePaymentIntent {
        amount: amt,
        currency: Currency::USD,
        metadata: Some(std::collections::HashMap::from([(
            String::from(PRODUCT_ID_KEY),
            product_id.to_string(),
        )])),
        on_behalf_of: None,
        transfer_data: None,
        application_fee_amount: None,
        automatic_payment_methods: None,
        capture_method: None,
        confirm: Some(false),
        customer: None,
        description: None,
        payment_method: None,
        receipt_email: None,
        return_url: None,
        setup_future_usage: None,
        shipping: None,
        statement_descriptor: None,
        statement_descriptor_suffix: None,
        transfer_group: None,
        use_stripe_sdk: None,
        mandate: None,
        mandate_data: None,
        off_session: None,
        payment_method_options: None,
        payment_method_types: None,
        confirmation_method: None,
        error_on_requires_action: None,
        expand: &[],
        payment_method_configuration: None,
        payment_method_data: None,
        radar_options: None,
    };

    let payment_intent = stripe::PaymentIntent::create(&client, create_pi)
        .await
        .map_err(|e| {
            ctx.to_ctx_error(AppError::Stripe {
                source: e.to_string(),
            })
        })?;

    Ok((StatusCode::OK, payment_intent.client_secret.unwrap()).into_response())
}

#[allow(dead_code)]
struct StripeEvent(Event);

#[async_trait]
impl<S> FromRequest<S> for StripeEvent
where
    String: FromRequest<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request<Body>, state: &S) -> Result<Self, Self::Rejection> {
        let signature = if let Some(sig) = req.headers().get("stripe-signature") {
            sig.to_owned()
        } else {
            return Err(StatusCode::BAD_REQUEST.into_response());
        };

        // TODO is state ctx_state so we can set from env in main
        let payload = String::from_request(req, state)
            .await
            .map_err(IntoResponse::into_response)?;

        let wh_secret =
            std::env::var("STRIPE_WEBHOOK_SECRET").expect("Missing STRIPE_WEBHOOK_SECRET in env");

        let event =
            stripe::Webhook::construct_event(&payload, signature.to_str().unwrap(), &wh_secret);

        match event {
            Ok(e) => Ok(Self(e)),
            Err(e) => {
                println!("Error constructing Stripe webhook event: {:?}", e);
                Err(StatusCode::BAD_REQUEST.into_response())
            }
        }
    }
}

async fn handle_webhook(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    StripeEvent(event): StripeEvent,
) -> CtxResult<Response> {
    // TODO why do we need variable outside of match - the match is not looping it's only called once
    let mut amount_received = 0;

    match event.type_ {
        stripe::EventType::PaymentIntentSucceeded => {
            println!("PaymentIntentSucceeded event received");
            if let stripe::EventObject::PaymentIntent(payment_intent) = event.data.object {
                // TODO make all values in db fixed with nr of decimals
                amount_received = (payment_intent.amount_received + amount_received) / 100;
                if amount_received <= 0 {
                    return Ok("No amount received".into_response());
                }

                let currency_symbol = CurrencySymbol::USD;

                println!("PaymentIntent ID: {}", payment_intent.id);
                println!("Amount Received: {}", amount_received);

                let fund_service = FundingTransactionDbService {
                    db: &ctx_state._db,
                    ctx: &ctx,
                };

                let metadata = &payment_intent.metadata;
                let endowment_id: Option<EndowmentIdent> = metadata
                    .get(PRODUCT_ID_KEY)
                    .and_then(|pr_id| {
                        println!("{pr_id}");
                        ProductId::from_str(pr_id).ok()
                    })
                    .and_then(|pr_id| {
                        dbg!("&pr_id");
                        EndowmentIdent::try_from(MyStripeProductId(pr_id)).ok()
                    });

                let user_id: Thing = match endowment_id {
                    Some(end_id) => end_id.user_id,
                    None => fund_service.unknown_endowment_user_id(),
                };

                let external_account = payment_intent.customer.as_ref().map_or(
                    "unknown_customer".to_string(),
                    |cust| match cust {
                        stripe::Expandable::Id(ref id) => id.as_str().to_string(),
                        stripe::Expandable::Object(ref obj) => obj.id.as_str().to_string(),
                    },
                );

                let external_tx_id = payment_intent.id.clone();

                fund_service
                    .user_endowment_tx(
                        &user_id,
                        external_account,
                        external_tx_id.to_string(),
                        amount_received,
                        currency_symbol,
                    )
                    .await
                    .expect("Full endowment created");

                let wallet_service = WalletDbService {
                    db: &ctx_state._db,
                    ctx: &ctx,
                };

                // TODO why we need to call get_user_balance?
                let user1_bal = wallet_service
                    .get_user_balance(&user_id)
                    .await
                    .expect("got balance");

                println!("Updated user balance {:?}", user1_bal);

                return Ok("Full payment processed".into_response());
            }
        }
        stripe::EventType::PaymentIntentPartiallyFunded => {
            println!("PaymentIntentPartiallyFunded event received");
            if let stripe::EventObject::PaymentIntent(payment_intent) = &event.data.object {
                // TODO payment_intent.amount_received is fixed value so need to divide by 100
                let partial_amount_received = payment_intent.amount_received;
                if partial_amount_received <= 0 {
                    return Ok("No partial amount received".into_response());
                }

                // TODO why are we setting outside variable?
                amount_received += partial_amount_received;

                println!("Partial Amount Received: {}", partial_amount_received);
                println!("Total Amount Received (accumulated): {}", amount_received);

                // TODO there is no user_endowment_tx call here - what's the purpose of this event type if not to make endowment_tx call for partial amount?
                return Ok("Partial payment processed".into_response());
            }
        }
        _ => {
            if ctx_state.is_development {
                println!("Unknown event encountered in webhook: {:?}", event.type_);
                // dbg!(event.data.object);
            }
        }
    }

    Ok("".into_response())
}
