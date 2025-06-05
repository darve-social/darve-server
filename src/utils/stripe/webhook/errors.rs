#[derive(Debug)]
pub enum WebhookError {
    BadKey,
    BadSignature,
    BadTimestamp(i64),
    ParseError(serde_json::Error),
    HeaderMissing,
}
