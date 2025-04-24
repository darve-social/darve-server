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
// use futures::TryFutureExt;
use stripe::{
    AccountId, Client, CreatePaymentIntent, CreatePrice, CreateProduct, Currency, Event,
    EventObject, EventType, Expandable, IdOrCreate, Invoice, Price, Product, ProductId,
};
// use stripe::resources::checkout::checkout_session_ext::RetrieveCheckoutSessionLineItems;
use surrealdb::sql::Thing;

use sb_middleware::ctx::Ctx;
use sb_middleware::error::{AppError, CtxError, CtxResult};
use sb_middleware::mw_ctx::CtxState;
use sb_middleware::utils::string_utils::get_string_thing;

const PRICE_USER_ID_KEY: &str = "user_id";

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
    amount: u32,
    pub action: String,
}

impl From<EndowmentIdent> for ProductId {
    fn from(value: EndowmentIdent) -> Self {
        let stripe_prod_id = format!(
            "{}_{}_{}",
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
        let mut spl = value.0.as_str().split("_");
        let user_ident = spl.next().ok_or(AppError::Generic {
            description: "missing user_id part".to_string(),
        })?;
        let amount = spl.next().ok_or(AppError::Generic {
            description: "missing amount part".to_string(),
        })?;
        let amount = amount.parse::<u32>().map_err(|e| AppError::Generic {
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

    let mut stripe_connect_account_id = ctx_state.stripe_platform_account.clone();
    println!("Stripe Connect Account ID: {}", stripe_connect_account_id);

    // taken from ctx_state/env
    if ctx_state.is_development {
        stripe_connect_account_id = "acct_1R3u5oJ2SO8dU9QJ".to_string();
    }

    let price_amount = amount;

    let acc_id = AccountId::from_str(stripe_connect_account_id.as_str()).map_err(|e1| {
        ctx.to_ctx_error(AppError::Stripe {
            source: e1.to_string(),
        })
    })?;
    let client = Client::new(ctx_state.stripe_key).with_stripe_account(acc_id.clone());

    // TODO product is not used in payment intent why are we creating it?
    // should just add product id to metadata
    let product = {
        let product_title = "wallet_endowment".to_string();
        let pr_id: ProductId = EndowmentIdent {
            user_id: get_string_thing(user_id.clone())?,
            amount: price_amount,
            action: product_title.clone(),
        }
        .into();
        let prod_res = Product::retrieve(&client, &pr_id, &[]).await;

        if prod_res.is_err() {
            let mut create_product = CreateProduct::new(product_title.as_str());
            create_product.id = Some(pr_id.as_str());
            Product::create(&client, create_product).await.unwrap()
        } else {
            prod_res.unwrap()
        }
    };

    let price = {
        let mut create_price = CreatePrice::new(Currency::USD);
        create_price.product = Some(IdOrCreate::Id(&product.id));
        create_price.metadata = Some(std::collections::HashMap::from([(
            String::from(PRICE_USER_ID_KEY),
            String::from(user_id.clone()),
        )]));
        create_price.unit_amount = Some((price_amount * 100) as i64);
        create_price.expand = &["product"];

        Price::create(&client, create_price).await.unwrap()
    };

    let amt = price
        .unit_amount
        .ok_or(ctx.to_ctx_error(AppError::Generic {
            description: "amount not set on product".to_string(),
        }))?;

    let create_pi = CreatePaymentIntent {
        amount: amt,
        currency: Currency::USD,
        metadata: Some(std::collections::HashMap::from([(
            String::from(PRICE_USER_ID_KEY),
            user_id.clone(),
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
    
        let payload = String::from_request(req, state)
            .await
            .map_err(IntoResponse::into_response)?;
    
        let wh_secret = std::env::var("STRIPE_WEBHOOK_SECRET").expect("Missing STRIPE_WEBHOOK_SECRET in env");
    
        let event = stripe::Webhook::construct_event(&payload, signature.to_str().unwrap(), &wh_secret);
    
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
    // match event.type_ {
        // TODO should use payment intent and metadata to determine endowment
        // EventType::InvoicePaid => {
        //     if let EventObject::Invoice(invoice) = event.data.object {
        //         let amount_paid = invoice.amount_paid.unwrap_or(0);
        //         if amount_paid <= 0 {
        //             return Ok("No amount paid".into_response());
        //         }

        //         let currency_symbol = CurrencySymbol::USD;
        //         let invoice_clone = invoice.clone();
        //         let endowments = extract_invoice_data(&ctx_state, &ctx, invoice_clone).await?;
        //         let external_account = match invoice.customer {
        //             Some(Expandable::Id(ref id)) => id.as_str().to_string(),
        //             Some(Expandable::Object(ref obj)) => obj.id.as_str().to_string(),
        //             None => "unknown_customer".to_string(),
        //         };
        //         let external_tx_id = invoice.id.clone();
        //         let fund_service = FundingTransactionDbService {
        //             db: &ctx_state._db,
        //             ctx: &ctx,
        //         };

        //         if invoice.amount_remaining.unwrap_or(0) > 0
        //             || invoice.paid.unwrap_or(false) == false
        //         {
        //             let partial_amount = amount_paid;

        //             fund_service
        //                 .user_endowment_tx(
        //                     &endowments[0].user_id,
        //                     external_account,
        //                     external_tx_id.to_string(),
        //                     partial_amount,
        //                     currency_symbol,
        //                 )
        //                 .await
        //                 .expect("Partial endowment created");

        //             return Ok("Partial payment processed".into_response());
        //         }

        //         let total_amount: i64 = endowments.iter().map(|e| e.amount as i64).sum();

        //         fund_service
        //             .user_endowment_tx(
        //                 &endowments[0].user_id,
        //                 external_account,
        //                 external_tx_id.to_string(),
        //                 total_amount,
        //                 currency_symbol,
        //             )
        //             .await
        //             .expect("Full endowment created");
        //     }
        // }
        // _ => {
            // if ctx_state.is_development {
            //     println!("Unknown event encountered in webhook: {:?}", event.type_);
            //     // dbg!(event.data.object);
            // }
        // }
    // }
    match event.type_ {
        stripe::EventType::PaymentIntentSucceeded => {
            println!("PaymentIntentSucceeded event received");
            if let stripe::EventObject::PaymentIntent(payment_intent) = event.data.object {
                let amount_received = payment_intent.amount_received;
                if amount_received <= 0 {
                    return Ok("No amount received".into_response());
                }


                let currency_symbol = CurrencySymbol::USD;

                println!("PaymentIntent ID: {}", payment_intent.id);
                println!("Amount Received: {}", amount_received);

                let metadata = &payment_intent.metadata;
                let user_id_str = metadata.get("user_id").unwrap_or(&"unknown".to_string()).clone();
                let user_id = get_string_thing(user_id_str)?;

                let external_account = payment_intent.customer.as_ref().map_or("unknown_customer".to_string(), |cust| {
                    match cust {
                        stripe::Expandable::Id(ref id) => id.as_str().to_string(),
                        stripe::Expandable::Object(ref obj) => obj.id.as_str().to_string(),
                    }
                });

                let external_tx_id = payment_intent.id.clone();
                let fund_service = FundingTransactionDbService {
                    db: &ctx_state._db,
                    ctx: &ctx,
                };

                if amount_received > 0 {
                    fund_service
                        .user_endowment_tx(
                            &user_id,
                            external_account,
                            external_tx_id.to_string(),
                            amount_received,
                            currency_symbol,
                        )
                        .await
                        .expect("Partial endowment created");

                    return Ok("Partial payment processed".into_response());
                }

                // If the full amount was received, process full endowment
                let total_amount = amount_received; // Adjust based on your use case
                fund_service
                    .user_endowment_tx(
                        &user_id,
                        external_account,
                        external_tx_id.to_string(),
                        total_amount,
                        currency_symbol,
                    )
                    .await
                    .expect("Full endowment created");

                return Ok("Full payment processed".into_response());
            }
        }
        _ => {
            if ctx_state.is_development {
                println!("Unknown event encountered in webhook: {:?}", event.type_);
                dbg!(event.data.object);
            }
        }
    }

    Ok("".into_response())
}

// async fn extract_invoice_data(
//     _ctx_state: &CtxState,
//     ctx: &Ctx,
//     invoice: Invoice,
// ) -> Result<Vec<EndowmentIdent>, CtxError> {
//     let mut endowments: Vec<EndowmentIdent> = vec![];
//     if let Some(list) = invoice.lines {
//         for item in list.data {
//             if let Some(price) = item.price {
//                 if let Some(mut md) = price.metadata {
//                     let user_id = md.remove(PRICE_USER_ID_KEY);
//                     if user_id.is_some() {
//                         let usr_id = get_string_thing(user_id.clone().unwrap());
//                         let product_id = price.product.unwrap().id();
//                         if usr_id.is_ok() {
//                             // product id has can be converted into EndowmentIdent that has userid info
//                             let endowment_ident: Result<EndowmentIdent, AppError> =
//                                 MyStripeProductId(product_id.clone()).try_into();
//                             if endowment_ident.is_ok() {
//                                 endowments.push(endowment_ident.expect("checked to be ok"));
//                             } else {
//                                 println!(
//                                     "ERROR stripe wh parse product id {} into thing invoice={}",
//                                     product_id.as_str(),
//                                     invoice.id.as_str()
//                                 )
//                             }
//                         } else {
//                             println!(
//                                 "ERROR stripe wh parse user id {:?} into thing invoice={}",
//                                 user_id.unwrap(),
//                                 invoice.id.as_str()
//                             )
//                         }
//                     } else {
//                         println!(
//                             "ERROR stripe wh no user id for price {} invoice={}",
//                             price.id.as_str(),
//                             invoice.id.as_str()
//                         );
//                     }
//                 }
//             }
//         }
//     };

//     if endowments.len() == 0 {
//         Err(ctx.to_ctx_error(AppError::Generic {
//             description: "extract invoice data err".to_string(),
//         }))
//     } else {
//         Ok(endowments)
//     }
// }
