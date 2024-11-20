use std::collections::HashMap;
use std::io::Write;
use std::str::FromStr;

use askama_axum::axum_core::response::IntoResponse;
use askama_axum::Template;
use axum::{async_trait, Router};
use axum::body::Body;
use axum::extract::{FromRequest, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::response::{Redirect, Response};
use axum::routing::{get, post};
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use stripe::{CreatePaymentLinkInvoiceCreation, CreatePaymentLinkInvoiceCreationInvoiceData, CreatePriceRecurring, CreatePriceRecurringInterval, EventObject, EventType, Invoice, ProductId};
use stripe::{Account, AccountId, AccountLink, AccountLinkType, AccountType, Client, CreateAccount, CreateAccountCapabilities, CreateAccountCapabilitiesCardPayments, CreateAccountCapabilitiesTransfers, CreateAccountLink, CreatePaymentLink, CreatePaymentLinkLineItems, CreatePrice, CreateProduct, Currency, Event, IdOrCreate, PaymentLink, Price, Product};
// use stripe::resources::checkout::checkout_session_ext::RetrieveCheckoutSessionLineItems;
use surrealdb::sql::{Id, Thing};
use tokio::io::AsyncWriteExt;

use sb_middleware::ctx::Ctx;
use sb_user_auth::entity::access_rule_entity::AccessRuleDbService;
use sb_user_auth::entity::authorization_entity::Authorization;
use sb_user_auth::entity::local_user_entity::LocalUserDbService;
use sb_user_auth::entity::access_gain_action_entitiy::{AccessGainActionType, AccessGainAction, AccessGainActionDbService, AccessGainActionStatus};
use sb_middleware::error::{CtxError, CtxResult, AppError};
use sb_middleware::mw_ctx::CtxState;
use crate::routes::community_routes::community_admin_access;
use sb_user_auth::routes::register_routes::display_register_page;
use sb_middleware::utils::db_utils::{IdentIdName, ViewFieldSelector};
use sb_middleware::utils::string_utils::get_string_thing;
use sb_user_auth::entity::access_right_entity::AccessRightDbService;
use crate::entity::community_entitiy::CommunityDbService;

const PRICE_USER_ID_KEY: &str = "user_id";

pub fn routes(state: CtxState) -> Router {
    Router::new()
        .route("/community/:community_id/stripe/link-start", get(get_link_start_page))
        .route("/community/:community_id/stripe/link-complete", get(get_link_complete_page))
        .route("/api/stripe/access-rule/:ar_id", get(access_rule_payment))
        .route("/api/stripe/webhook", post(handle_webhook))
        .with_state(state)
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/stripe_link_start.html")]
pub struct StripeLinkStartPage {
    theme_name: String,
    window_title: String,
    nav_top_title: String,
    header_title: String,
    footer_text: String,
    title: String,
    stripe_link: String,
    pub requirements_due: bool,
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/stripe_link_complete.html")]
pub struct StripeLinkCompletePage {
    theme_name: String,
    window_title: String,
    nav_top_title: String,
    header_title: String,
    footer_text: String,
    title: String,
    pub link: String,
}

#[derive(Deserialize)]
pub struct AccessRuleChargeView {
    pub id: Thing,
    pub community: Thing,
    pub title: String,
    // can use low authorize_height values for subscriptions (for ex. 0-1000000) so plans can be compared with .ge() and exact authorize_height (for ex. 1000000+) for product id comparison
    // can add functionality so .ge() comparison is below 1000000 and if required authorize_height is 1000000+ then exact check is made
    pub authorization_required: Authorization,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_amount: Option<i32>,
    // how long delivery is possible - how long subsctiption lasts
    pub available_period_days: Option<u64>,
    pub stripe_connect_account_id: Option<String>,
    pub stripe_connect_complete: bool,
}

impl ViewFieldSelector for AccessRuleChargeView {
    // post fields selct qry for view
    fn get_select_query_fields(_ident: &IdentIdName) -> String {
        "id, community, title, authorization_required, price_amount, available_period_days, community.*.stripe_connect_account_id as stripe_connect_account_id, community.*.stripe_connect_complete as stripe_connect_complete".to_string()
    }
}

async fn get_link_start_page(
    State(ctx_state): State<CtxState>,
    Path(community_id): Path<String>,
    ctx: Ctx,
) -> CtxResult<StripeLinkStartPage> {
    println!("->> {:<12} - get_link_start_page", "HANDLER");

    let (comm_id, mut comm) = community_admin_access(&ctx_state._db, &ctx, community_id).await?;

    if ctx_state.is_development {
        comm.stripe_connect_account_id = Some("acct_1Q29UUEdDBSaSZL3".to_string());
    }

    let client = Client::new(ctx_state.stripe_key);

    let connect_account_id = match comm.stripe_connect_account_id.clone() {
        None => {
            let acc: Account = Account::create(&client,
                                               CreateAccount {
                                                   type_: Some(AccountType::Standard),
                                                   capabilities: Some(CreateAccountCapabilities {
                                                       card_payments: Some(CreateAccountCapabilitiesCardPayments { requested: Some(true) }),
                                                       transfers: Some(CreateAccountCapabilitiesTransfers { requested: Some(true) }),
                                                       ..Default::default()
                                                   }),
                                                   ..Default::default()
                                               })
                .map_err(|e| ctx.to_ctx_error(e.into()))
                .await?;
            comm.stripe_connect_account_id = Some(acc.id.clone().to_string());
            comm = CommunityDbService { ctx: &ctx, db: &ctx_state._db }.create_update(comm).await?;
            acc.id
        }
        Some(id) => AccountId::from_str(id.as_str()).map_err(|e1| ctx.to_ctx_error(AppError::Stripe { source: e1.to_string() }))?
    };

    let requirements_due = get_account_requirements_due(&client, &ctx, &connect_account_id).await?;
    let mut link = "".to_string();

    if requirements_due {
        link = AccountLink::create(
            &client,
            CreateAccountLink {
                account: connect_account_id,
                type_: AccountLinkType::AccountOnboarding,
                collect: None,
                expand: &[],
                refresh_url: Some(format!("http://localhost:8080/community/{}/stripe/link-start", comm_id.clone().to_raw()).as_str()),
                return_url: Some(format!("http://localhost:8080/community/{}/stripe/link-complete", comm_id.to_raw()).as_str()),
                collection_options: None,
            }, )
            .map_err(|e| ctx.to_ctx_error(e.into()))
            .await?.url;
    } else {
        if comm.stripe_connect_complete == false {
            comm.stripe_connect_complete = true;
            CommunityDbService { ctx: &ctx, db: &ctx_state._db }.create_update(comm).await?;
        }
    }

    Ok(StripeLinkStartPage {
        theme_name: "emerald".to_string(),
        window_title: "edit access rule".to_string(),
        nav_top_title: "edit".to_string(),
        header_title: "ttl".to_string(),
        footer_text: "fedit".to_string(),
        title: "another ttl".to_string(),
        stripe_link: link,
        requirements_due,
    })
}

async fn get_link_complete_page(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(community_id): Path<String>,
) -> CtxResult<StripeLinkCompletePage> {
    println!("->> {:<12} - get_link_complete_page ", "HANDLER");

    let (comm_id, mut comm) = community_admin_access(&ctx_state._db, &ctx, community_id).await?;

    if ctx_state.is_development {
        comm.stripe_connect_account_id = Some("acct_1Q29UUEdDBSaSZL3".to_string());
    }

    let link = if comm.stripe_connect_account_id.is_none() {
        // return Err(ctx.to_api_error(Error::Generic { description: "No Stripe account conncted".to_string() }));
        format!("/community/{}/stripe/link-start", comm_id.clone().to_raw())
    } else {
        let acc_id = AccountId::from_str(comm.stripe_connect_account_id.clone().unwrap().as_str()).map_err(|e1| ctx.to_ctx_error(AppError::Stripe { source: e1.to_string() }))?;
        let requirements_due = get_account_requirements_due(&Client::new(ctx_state.stripe_key), &ctx, &acc_id).await?;

        if requirements_due {
            format!("/community/{}/stripe/link-start", comm_id.clone().to_raw())
        } else {
            if comm.stripe_connect_complete == false {
                comm.stripe_connect_complete = true;
                CommunityDbService { ctx: &ctx, db: &ctx_state._db }.create_update(comm).await?;
            }
            "".to_string()
        }
    };

    Ok(StripeLinkCompletePage {
        theme_name: "emerald".to_string(),
        window_title: "edit access rule".to_string(),
        nav_top_title: "edit".to_string(),
        header_title: "ttl".to_string(),
        footer_text: "fedit".to_string(),
        title: "another ttl".to_string(),
        link,
    })
}

struct MyThing(Thing);

impl From<MyThing> for ProductId {
    fn from(value: MyThing) -> Self {
        let stripe_prod_id = value.0.to_raw().replace(":", "-");
        ProductId::from_str(stripe_prod_id.as_str()).unwrap()
    }
}

struct MyStripeProductId(ProductId);

impl TryFrom<MyStripeProductId> for Thing {
    type Error = AppError;

    fn try_from(value: MyStripeProductId) -> Result<Self, Self::Error> {
        let thing_ident = value.0.as_str().replace("-", ":");
        Thing::try_from(thing_ident).map_err(|e| Self::Error::Generic { description: "Can not convert to ident from stripe product id".to_string() })
    }
}


async fn access_rule_payment(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    Path(access_rule_id): Path<String>,
) -> CtxResult<Response> {
    println!("->> {:<12} - access_rule_payment ", "HANDLER");

    if ctx.user_id().is_err() {
        let mut qry: HashMap<String, String> = HashMap::new();
        qry.insert("next".to_string(), format!("/api/stripe/access-rule/{access_rule_id}"));
        return Ok(display_register_page(ctx, Query(qry)).await?.into_response());
    }

    let user_id = ctx.user_id()?;

    let mut charge_access_rule = AccessRuleDbService { db: &ctx_state._db, ctx: &ctx }.get_view::<AccessRuleChargeView>(IdentIdName::Id(get_string_thing(access_rule_id.clone())?)).await?;

    if ctx_state.is_development {
        charge_access_rule.stripe_connect_account_id = Some("acct_1Q29UUEdDBSaSZL3".to_string());
    }

    if charge_access_rule.price_amount.is_none() {
        return Err(ctx.to_ctx_error(AppError::Generic { description: "Price not defined".to_string() }));
    }


    if !ctx_state.is_development && (charge_access_rule.stripe_connect_account_id.is_none() || !charge_access_rule.stripe_connect_complete) {
        return Err(ctx.to_ctx_error(AppError::Generic { description: "No Stripe account conncted".to_string() }));
    }

    let acc_id = AccountId::from_str(charge_access_rule.stripe_connect_account_id.clone().unwrap().as_str()).map_err(|e1| ctx.to_ctx_error(AppError::Stripe { source: e1.to_string() }))?;
    let client = Client::new(ctx_state.stripe_key).with_stripe_account(acc_id.clone());

    let product = {
        let pr_id: ProductId = MyThing(charge_access_rule.id).into();
        let prod_res = Product::retrieve(&client, &pr_id, &[]).await;


        if prod_res.is_err() {
            let mut create_product = CreateProduct::new(charge_access_rule.title.as_str());
            create_product.id = Some(pr_id.as_str());
            Product::create(&client, create_product).await.unwrap()
        } else { prod_res.unwrap() }
        /*create_product.metadata = Some(std::collections::HashMap::from([(
            String::from("access-rule-id"),
            access_rule_id,
        ),(
            String::from("user-id"),
            user_id,
        )]));*/
    };

    // and add a price for it in USD
    let price = {
        let mut create_price = CreatePrice::new(Currency::USD);
        create_price.product = Some(IdOrCreate::Id(&product.id));
        create_price.metadata = Some(std::collections::HashMap::from([(
            String::from(PRICE_USER_ID_KEY),
            String::from(user_id.clone()),
        )]));
        create_price.unit_amount = Some((charge_access_rule.price_amount.unwrap() * 100) as i64);
        create_price.expand = &["product"];
        create_price.recurring = match charge_access_rule.available_period_days {
            Some(days_interval) => {
                match days_interval > 0 {
                    true => Some(CreatePriceRecurring {
                        aggregate_usage: None,
                        interval: CreatePriceRecurringInterval::Day,
                        interval_count: Some(days_interval),
                        trial_period_days: None,
                        usage_type: None,
                    }),
                    false => None
                }
            }
            None => None
        };

        Price::create(&client, create_price).await.unwrap()
    };

    // println!(
    //     "created a product {:?} at price {} {}",
    //     product.name.unwrap(),
    //     price.unit_amount.unwrap() / 100,
    //     price.currency.unwrap()
    // );

    let platform_fee = match price.unit_amount {
        None => ctx_state.min_platform_fee_abs_2dec as i64,
        Some(amt) => {
            let rel = (amt as f64 * ctx_state.platform_fee_rel as f64) as i64;
            println!("FEEE {} // {}", rel, amt);
            if rel < ctx_state.min_platform_fee_abs_2dec as i64 {
                ctx_state.min_platform_fee_abs_2dec as i64
            } else { rel }
        }
    };

    println!("PLATFRM FEE={}", platform_fee);

    let create_pl = CreatePaymentLink {
        after_completion: None,
        allow_promotion_codes: None,
        line_items: vec![CreatePaymentLinkLineItems {
            quantity: 1,
            price: price.id.to_string(),
            ..Default::default()
        }],
        metadata: Some(std::collections::HashMap::from([(
            String::from(PRICE_USER_ID_KEY),
            user_id.clone(),
        )])),
        // on_behalf_of:Some(acc_id.as_str()),
        on_behalf_of: None,
        payment_intent_data: None,
        payment_method_collection: None,
        payment_method_types: None,
        phone_number_collection: None,
        restrictions: None,
        shipping_address_collection: None,
        shipping_options: None,
        submit_type: None,
        subscription_data: None,
        tax_id_collection: None,
        // transfer_data: Some(CreatePaymentLinkTransferData{destination: acc_id.to_string(), ..Default::default()}),
        transfer_data: None,
        automatic_tax: None,
        billing_address_collection: None,
        consent_collection: None,
        currency: None,
        custom_fields: None,
        custom_text: None,
        customer_creation: None,
        expand: &[],
        inactive_message: None,
        // value with 2 decimals - 1500 is $15s
        application_fee_amount: match price.recurring.is_some() {
            true => None,
            false => Some(platform_fee)
        },
        // subscriptions only=  application_fee_percent: Some(10f64),
        application_fee_percent: match price.recurring.is_some() {
            false => None,
            true => Some((ctx_state.platform_fee_rel * 100 as f64) as f64)
        },
        invoice_creation: match price.recurring.is_some() {
            true => None,
            false => Some(CreatePaymentLinkInvoiceCreation {
                enabled: true,
                invoice_data: Some(CreatePaymentLinkInvoiceCreationInvoiceData {
                    ..Default::default()
                }),
            })
        },
    };

    let mut payment_link = PaymentLink::create(
        &client,
        create_pl,
    )
        .await.map_err(|e| ctx.to_ctx_error(AppError::Stripe { source: e.to_string() }))?;

    /*let action_id =
        PaymentActionDbService { ctx: &ctx, db: &ctx_state._db }.create_update(PaymentAction {
            id: Thing::from((PaymentActionDbService::get_table_name(), Id::from(payment_link.id.as_str()))),
            community: charge_access_rule.community,
            access_rules: vec![charge_access_rule.id],
            local_user: Thing::try_from(user_id).map_err(|e| ctx.to_api_error(Error::Generic { description: "error into access_rule Thing".to_string() }))?,
            paid: false,
            r_created: None,
            r_updated: None,
        }).await?;*/

    /*let mut payment_link = PaymentLink::create(
        &client,
        CreatePaymentLink::new(vec![CreatePaymentLinkLineItems {
            quantity: 3,
            price: price.id.to_string(),
            ..Default::default()
        }]),
    )
        .await.unwrap();*/

    Ok(Redirect::temporary(payment_link.url.as_str()).into_response())
}


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

        let payload =
            String::from_request(req, state).await.map_err(IntoResponse::into_response)?;

        let wh_secret = "whsec_09294dbed5e920d70bfbceeb507014faabc29f94658e4d643fea98d21978cb38";
        Ok(Self(
            stripe::Webhook::construct_event(&payload, signature.to_str().unwrap(), wh_secret)
                .map_err(|_| StatusCode::BAD_REQUEST.into_response())?,
        ))
    }
}

async fn handle_webhook(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    StripeEvent(event): StripeEvent,
) -> CtxResult<Response> {
    match event.type_ {
        /*EventType::CheckoutSessionCompleted => {
            if let EventObject::CheckoutSession(session) = event.data.object {
                println!("Received checkout session completed webhook with id: {:?}", session.id);
                // dbg!(&session);

                if let Some(status) = session.status {
                    if status == CheckoutSessionStatus::Complete {
                        dbg!(&session);
                        if let Some(payment_link_id) = session.payment_link {
                            let payment_link_id = payment_link_id.id().to_string();
                            let payment_action_db_service = PaymentActionDbService { ctx: &ctx, db: &ctx_state._db };
                            let mut p_action = payment_action_db_service.get(IdentIdName::Id(format!("{}:{payment_link_id}", PaymentActionDbService::get_table_name()))).await?;
                            p_action.paid = true;
                            let p_action = payment_action_db_service.create_update(p_action).await?;
                            // dbg!(&p_action);
                            for access_rule in p_action.access_rules {
                                let a_right = LocalUserDbService { ctx: &ctx, db: &ctx_state._db }.add_paid_access_right(p_action.local_user.clone(), access_rule, p_action.id.clone()).await?;
                                // dbg!(a_right);
                            }
                        }
                    }
                }
                // let sess = CheckoutSession::retrieve(&Client::new(ctx_state.stripe_key).with_stripe_account(AccountId::from_str("acct_1Q29UUEdDBSaSZL3").unwrap()), &session.id, &["line_items"]).await;
                // dbg!(sess);
                /* let items = CheckoutSession::retrieve_line_items(&Client::new(ctx_state.stripe_key), &session.id, &RetrieveCheckoutSessionLineItems{
                     ending_before: None,
                     limit: None,
                     starting_after: None,
                 }).await;*/

                /*for item in session.line_items.unwrap().data {
                    let product = item.price.unwrap().product.unwrap().into_object().unwrap();
                    dbg!(&product.id);
                    dbg!(&product.metadata);
                    println!("/////////////");
                }*/
            }
        }*/
        EventType::InvoicePaymentFailed => {
            if let EventObject::Invoice(invoice) = event.data.object {
                let external_ident = Some(invoice.id.as_str().clone().to_string());
                let invoice_rules = extract_invoice_data(&ctx_state, &ctx, invoice).await?;
                let u_id = invoice_rules.get(0).expect("invoice should have items").0.clone();

                AccessGainActionDbService { db: &ctx_state._db, ctx: &ctx }.create_update(AccessGainAction {
                    id: None,
                    external_ident,
                    access_rule_pending: None,
                    access_rights: None,
                    local_user: Option::from(u_id),
                    action_type: AccessGainActionType::Stripe,
                    action_status: AccessGainActionStatus::Failed,
                    r_created: None,
                    r_updated: None,
                }).await?;
            }
        }
        EventType::InvoicePaid => {
            if let EventObject::Invoice(invoice) = event.data.object {
                // dbg!(&invoice);
                let id = Thing::try_from((AccessGainActionDbService::get_table_name().to_string(), Id::from(invoice.id.as_str()))).unwrap();
                let j_action_db = AccessGainActionDbService { db: &ctx_state._db, ctx: &ctx };
                let mut j_action = AccessGainAction {
                    id: None,
                    external_ident: Some(invoice.id.to_string()),
                    access_rule_pending: None,
                    access_rights: None,
                    local_user: None,
                    action_type: AccessGainActionType::Stripe,
                    action_status: AccessGainActionStatus::Failed,
                    r_created: None,
                    r_updated: None,
                };
                if (invoice.amount_remaining.is_some() && invoice.amount_remaining.unwrap().gt(&0))
                    || invoice.paid.is_none() || invoice.paid.unwrap() == false {
                    j_action_db.create_update(j_action).await?;
                    // don't process partially paid
                    return Ok(StatusCode::OK.into_response());
                }

                let invoice_rules = extract_invoice_data(&ctx_state, &ctx, invoice).await?;
                let mut a_rights = vec![];
                j_action.local_user = Some(invoice_rules.get(0).expect("invoice must have items").0.clone());
                j_action.action_status = AccessGainActionStatus::Complete;
                j_action = j_action_db.create_update(j_action).await?;
                for user_a_rule in invoice_rules {
                    let (usr_id, a_rule_thing) = user_a_rule;
                    let a_right = AccessRightDbService { ctx: &ctx, db: &ctx_state._db }.add_paid_access_right(usr_id.clone(), a_rule_thing.clone(), j_action.id.clone().expect("must be saved already, having id")).await?;
                    a_rights.push(a_right.id.expect("AccessRight to be saved"));
                }
                j_action.access_rights = Some(a_rights);
                j_action_db.create_update(j_action).await?;
            }
        }
        /*EventType::SubscriptionScheduleCreated => {
            if let EventObject::SubscriptionSchedule(subs_sched) = event.data.object {
                println!("Received subscription schedule webhook: {:?}", subs_sched.id);
                dbg!(subs_sched);
            }
        }
        EventType::AccountUpdated => {
            if let EventObject::Account(account) = event.data.object {
                println!("Received account updated webhook for account: {:?}", account.id);
            }
        }*/
        _ => {
            if ctx_state.is_development {
                println!("Unknown event encountered in webhook: {:?}", event.type_);
            }
        }
    }
    Ok("".into_response())
}

async fn extract_invoice_data(_ctx_state: &CtxState, ctx: &Ctx, invoice: Invoice) -> Result<Vec<(Thing, Thing)>, CtxError> {
    let mut user_access_rules: Vec<(Thing, Thing)> = vec![];
    if let Some(list) = invoice.lines {
        for item in list.data {
            if let Some(price) = item.price {
                if let Some(mut md) = price.metadata {
                    let user_id = md.remove(PRICE_USER_ID_KEY);
                    if user_id.is_some() {
                        let usr_id = Thing::try_from(user_id.clone().unwrap());
                        let product_id = price.product.unwrap().id();
                        if usr_id.is_ok() {
                            let access_rule_thing: Result<Thing, AppError> = MyStripeProductId(product_id.clone()).try_into();
                            if access_rule_thing.is_ok() {
                                user_access_rules.push((usr_id.unwrap(), access_rule_thing.unwrap()));
                                // return Ok((usr_id.unwrap(), access_rule_thing.unwrap()));
                            } else {
                                println!("ERROR stripe wh parse product id {} into thing invoice={}", product_id.as_str(), invoice.id.as_str())
                            }
                        } else { println!("ERROR stripe wh parse user id {:?} into thing invoice={}", user_id.unwrap(), invoice.id.as_str()) }
                    } else { println!("ERROR stripe wh no user id for price {} invoice={}", price.id.as_str(), invoice.id.as_str()); }
                }
            }
        }
    };

    if user_access_rules.len() == 0 {
        Err(ctx.to_ctx_error(AppError::Generic { description: "extract invoice data err".to_string() }))
    } else { Ok(user_access_rules) }
}

async fn get_account_requirements_due(client: &Client, ctx: &Ctx, connect_account_id: &AccountId) -> CtxResult<bool> {
    let acc: Account = Account::retrieve(client, &connect_account_id, Default::default())
        .map_err(|e| ctx.to_ctx_error(e.into())).await?;
    // dbg!(&acc);

    let mut requirements_due = false;

    if let Some(req) = acc.requirements {
        if let Some(past_due) = req.past_due {
            if past_due.len() > 0 {
                requirements_due = true;
            }
        }
        if let Some(curr_due) = req.currently_due {
            if curr_due.len() > 0 {
                requirements_due = true;
            }
        }
    };
    Ok(requirements_due)
}
