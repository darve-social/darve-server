use axum::{
    http::header,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::sync::Arc;

use crate::middleware::mw_ctx::CtxState;

pub fn routes() -> Router<Arc<CtxState>> {
    Router::new()
        .route("/api/docs", get(swagger_ui))
        .route("/api/docs/openapi.json", get(openapi_spec))
}

async fn swagger_ui() -> impl IntoResponse {
    let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Darve API Documentation</title>
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5.9.0/swagger-ui.css" />
    <style>
        html {
            box-sizing: border-box;
            overflow: -moz-scrollbars-vertical;
            overflow-y: scroll;
        }
        *, *:before, *:after {
            box-sizing: inherit;
        }
        body {
            margin:0;
            background: #fafafa;
        }
    </style>
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5.9.0/swagger-ui-bundle.js"></script>
    <script src="https://unpkg.com/swagger-ui-dist@5.9.0/swagger-ui-standalone-preset.js"></script>
    <script>
        window.onload = function() {
            const ui = SwaggerUIBundle({
                url: '/api/docs/openapi.json',
                dom_id: '#swagger-ui',
                deepLinking: true,
                presets: [
                    SwaggerUIBundle.presets.apis,
                    SwaggerUIStandalonePreset
                ],
                plugins: [
                    SwaggerUIBundle.plugins.DownloadUrl
                ],
                layout: "StandaloneLayout",
                tryItOutEnabled: true,
                requestInterceptor: (request) => {
                    // Add any request interceptors here if needed
                    return request;
                },
                responseInterceptor: (response) => {
                    // Add any response interceptors here if needed
                    return response;
                }
            });
        };
    </script>
</body>
</html>
    "#;

    Html(html)
}

async fn openapi_spec() -> impl IntoResponse {
    let openapi_json = include_str!("../openapi.json");

    ([(header::CONTENT_TYPE, "application/json")], openapi_json)
}
