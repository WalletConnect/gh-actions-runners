use github::webhook::handle_webhook;
use jsonwebtoken::EncodingKey;
use lambda_http::{
    http::{Response, StatusCode},
    run, service_fn, Error, Request,
};
use octocrab::{models::AppId, Octocrab};
use serde_json::json;
use tracing::error;

pub mod ecs;
pub mod github;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .without_time()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app_id = std::env::var("GITHUB_APP_ID").unwrap();
    let private_key = std::env::var("GITHUB_APP_PRIVATE_KEY").unwrap();
    octocrab::initialise(
        Octocrab::builder()
            .app(
                AppId::from(app_id.parse::<u64>().unwrap()),
                EncodingKey::from_rsa_pem(private_key.as_bytes()).unwrap(),
            )
            .build()
            .unwrap(),
    );

    run(service_fn(function_handler)).await
}

pub fn build_response(status: StatusCode) -> Result<Response<String>, Error> {
    Ok(Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(
            json!({
                "status": status.to_string(),
            })
            .to_string(),
        )
        .map_err(Box::new)?)
}

pub async fn function_handler(event: Request) -> Result<Response<String>, Error> {
    if event.method() != "POST" {
        return build_response(StatusCode::METHOD_NOT_ALLOWED);
    }

    if event.uri().path() != "/v1/webhook" {
        return build_response(StatusCode::NOT_FOUND);
    }

    let (parts, body) = event.into_parts();
    let headers = parts.headers;

    match handle_webhook(headers, body).await {
        Ok(response) => Ok(response),
        Err(e) => {
            error!("error handling webhook: {e:?}");
            build_response(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
