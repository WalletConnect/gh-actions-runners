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

#[cfg(test)]
mod jwt_backend_tests {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    use serde::Serialize;

    #[derive(Serialize)]
    struct Claims {
        iss: String,
        iat: u64,
        exp: u64,
    }

    /// Octocrab's `.app()` path signs GitHub App JWTs with RS256 from an RSA PEM.
    /// Guards the jsonwebtoken `jwt-aws-lc-rs` backend swap.
    #[test]
    fn rs256_signing_from_rsa_pem_works() {
        let rsa = openssl::rsa::Rsa::generate(2048).expect("generate RSA key");
        let pem = rsa.private_key_to_pem().expect("encode PEM");

        let key = EncodingKey::from_rsa_pem(&pem).expect("from_rsa_pem");
        let token = encode(
            &Header::new(Algorithm::RS256),
            &Claims {
                iss: "12345".into(),
                iat: 1_700_000_000,
                exp: 1_700_000_600,
            },
            &key,
        )
        .expect("RS256 encode");

        assert_eq!(token.matches('.').count(), 2, "expected 3 JWT segments");
        assert!(
            !token.split('.').nth(2).unwrap().is_empty(),
            "empty signature"
        );
        println!("signed JWT ok, {} bytes", token.len());
    }
}
