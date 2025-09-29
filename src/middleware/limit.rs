use axum::{extract::Request, response::Response};
use governor::middleware::NoOpMiddleware;
use reqwest::StatusCode;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::KeyExtractor, GovernorError, GovernorLayer,
};
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RobustIpKeyExtractor;

impl KeyExtractor for RobustIpKeyExtractor {
    type Key = String;

    fn extract<B>(&self, req: &Request<B>) -> Result<Self::Key, GovernorError> {
        let ip = req
            .headers()
            .get("X-Forwarded-For")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.split(',').next())
            .map(|s| s.trim())
            .or_else(|| {
                req.headers()
                    .get("X-Real-IP") // Nginx
                    .and_then(|h| h.to_str().ok())
            })
            .or_else(|| {
                req.headers()
                    .get("X-Client-IP") // Proxies
                    .and_then(|h| h.to_str().ok())
            })
            .or_else(|| {
                req.headers()
                    .get("CF-Connecting-IP") // Cloudflare
                    .and_then(|h| h.to_str().ok())
            })
            .or_else(|| {
                req.headers()
                    .get("True-Client-IP") // Akamai
                    .and_then(|h| h.to_str().ok())
            })
            .or_else(|| {
                req.headers()
                    .get("X-Originating-IP")
                    .and_then(|h| h.to_str().ok())
            })
            .or_else(|| {
                req.headers()
                    .get("X-Remote-IP")
                    .and_then(|h| h.to_str().ok())
            })
            .or_else(|| {
                req.headers()
                    .get("X-Remote-Addr")
                    .and_then(|h| h.to_str().ok())
            });

        if let Some(ip) = ip {
            return Ok(ip.to_string());
        }
        if let Some(addr) = req.extensions().get::<std::net::SocketAddr>() {
            return Ok(addr.ip().to_string());
        }
        Ok("unknown".to_string())
    }
}

pub fn create_rate_limit_layer(
    rps: u32,
    burst: u32,
) -> GovernorLayer<RobustIpKeyExtractor, NoOpMiddleware, axum::body::Body> {
    let config = GovernorConfigBuilder::default()
        .per_second(rps as u64)
        .burst_size(burst)
        .key_extractor(RobustIpKeyExtractor)
        .finish()
        .expect("Failed to create rate limit configuration");
    GovernorLayer::new(config).error_handler(|_e| {
        Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .body("Rate limit exceeded".into())
            .unwrap()
    })
}
