use std::io;
use std::io::ErrorKind::AlreadyExists;
use std::path::PathBuf;

use askama_axum::axum_core::response::IntoResponse;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::post;
use axum::{BoxError, Router};
use futures::{Stream, TryStreamExt};
use futures::TryFutureExt;
use tokio::fs::File;
use tokio::io::BufWriter;
use tokio_util::io::StreamReader;

use sb_middleware::ctx::Ctx;
use sb_middleware::error::{AppError, CtxResult};
use sb_middleware::mw_ctx::CtxState;

pub async fn routes(state: CtxState, uploads_dir: &str) -> Router {
    match tokio::fs::create_dir(uploads_dir).await {
        Ok(_) => println!("uploads dir created"),
        Err(err) if err.kind() == AlreadyExists => {
            println!("uploads dir already exists");
        }
        _ => {
            panic!("error creating uploads dir");
        }
    }

    // .expect("failed to create `uploads` directory");

    Router::new()
        .route("/api/upload", post(upload))
        .layer(DefaultBodyLimit::max(4000000))
        .with_state(state)
}

async fn upload(
    State(ctx_state): State<CtxState>,
    ctx: Ctx,
    mut multipart: Multipart,
) -> CtxResult<Response> {
    // let rrrrrr = multipart.next_field().await;
    // dbg!(rrrrrr.unwrap_err().source().unwrap());
    // match rrrrrr.unwrap_err() { MultipartError { .. } => {} }
    // return Ok((StatusCode::OK, "oooo".to_string()).into_response());
    // let mp=multipart.next_field().await.map_err(|e|ctx.to_api_error(Error::Generic {description:e.to_string()}))?;

    while let Ok(Some(field)) = multipart.next_field().await {
        // dbg!(&field.name());
        let file_name = if let Some(file_name) = field.file_name() {
            file_name.to_owned()
        } else {
            continue;
        };
        if !path_is_valid(&file_name) {
            return Err(ctx.to_ctx_error(AppError::Generic {
                description: "path not valid".to_string(),
            }));
        }
        let path = std::path::Path::new(ctx_state.uploads_dir.as_str()).join(&file_name);
        let saved = stream_to_file(path.clone(), field)
            .map_err(|e| ctx.to_ctx_error(e))
            .await?;
        dbg!(saved);
    }

    Ok((StatusCode::OK, "Uploaded").into_response())
}

async fn stream_to_file<S, E>(path: PathBuf, stream: S) -> Result<(String, u64), AppError>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>,
{
    async {
        // Convert the stream into an `AsyncRead`.
        let body_with_io_error = stream.map_err(|err| io::Error::new(io::ErrorKind::Other, err));
        let body_reader = StreamReader::new(body_with_io_error);
        futures::pin_mut!(body_reader);

        // Create the file. `File` implements `AsyncWrite`.
        // let path = std::path::Path::new(upload_dir).join(path);
        let file = File::create(&path).await?;
        let mut file_writer = BufWriter::new(file);

        // Copy the body into the file.
        let cp = tokio::io::copy(&mut body_reader, &mut file_writer).await?;

        Ok::<_, io::Error>((path.to_str().unwrap().to_string(), cp))
    }
    .await
    .map_err(|err| AppError::Generic {
        description: err.to_string(),
    })
}

// to prevent directory traversal attacks we ensure the path consists of exactly one normal
// component
fn path_is_valid(path: &str) -> bool {
    let path = std::path::Path::new(path);
    let mut components = path.components().peekable();

    if let Some(first) = components.peek() {
        if !matches!(first, std::path::Component::Normal(_)) {
            return false;
        }
    }

    components.count() == 1
}
