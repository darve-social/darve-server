use super::event::HookEvent;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashMap;

#[derive(Debug)]
pub enum WebhookError {
    BadKey,
    BadSignature,
    BadTimestamp(i64),
    ParseError(serde_json::Error),
    HeaderMissing,
}

fn parse_signature_header(sig_header: &str) -> Result<(i64, String), WebhookError> {
    let pairs: HashMap<_, _> = sig_header
        .split(',')
        .filter_map(|part| {
            let mut kv = part.split('=');
            Some((kv.next()?, kv.next()?))
        })
        .collect();

    let timestamp = pairs
        .get("t")
        .ok_or(WebhookError::HeaderMissing)?
        .parse::<i64>()
        .map_err(|_| WebhookError::BadSignature)?;

    let signature = pairs
        .get("v1")
        .ok_or(WebhookError::BadSignature)?
        .to_string();

    Ok((timestamp, signature))
}

pub fn verify_and_parse_event(
    payload: &str,
    sig_header: &str,
    secret: &str,
) -> Result<HookEvent, WebhookError> {
    let (timestamp, signature) = parse_signature_header(sig_header)?;
    let signed_payload = format!("{}.{}", timestamp, payload);

    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| WebhookError::BadKey)?;
    mac.update(signed_payload.as_bytes());

    let signature_bytes = hex::decode(signature).map_err(|_| WebhookError::BadSignature)?;
    mac.verify_slice(&signature_bytes)
        .map_err(|_| WebhookError::BadSignature)?;

    if (Utc::now().timestamp() - timestamp).abs() > 300 {
        return Err(WebhookError::BadTimestamp(timestamp));
    }
    Ok(serde_json::from_str(payload).map_err(|e| WebhookError::ParseError(e))?)
}
