use crate::{
    build_response, ecs::spawn_runner, github::runner_registration::get_runner_registration_token,
};
use constant_time_eq::constant_time_eq;
use hmac::{Hmac, Mac};
use http::{HeaderMap, StatusCode};
use lambda_http::{Body, Error, Response};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
struct Payload {
    action: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PayloadWithJob {
    action: String,
    workflow_job: WorkflowJob,
    repository: Repository,
}

#[derive(Debug, Serialize, Deserialize)]
struct WorkflowJob {
    labels: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Repository {
    name: String,
    owner: Owner,
}

#[derive(Debug, Serialize, Deserialize)]
struct Owner {
    login: String,
}

pub async fn handle_webhook(headers: HeaderMap, body: Body) -> Result<Response<String>, Error> {
    // https://docs.github.com/en/webhooks/using-webhooks/validating-webhook-deliveries
    let signature = headers
        .get("X-Hub-Signature-256")
        .unwrap()
        .to_str()
        .unwrap();
    let signature = signature.strip_prefix("sha256=").unwrap();
    let signature = hex::decode(signature).unwrap();
    let github_webhook_secret = std::env::var("GITHUB_WEBHOOK_SECRET").unwrap();
    let mut mac = Hmac::<Sha256>::new_from_slice(github_webhook_secret.as_bytes()).unwrap();
    mac.update(&body);
    let result = mac.finalize().into_bytes();
    if !constant_time_eq(&result[..], &signature[..]) {
        return build_response(StatusCode::UNAUTHORIZED);
    }

    info!("webhook received: {:?}", String::from_utf8_lossy(&body));

    let payload = serde_json::from_slice::<Payload>(&body).unwrap();

    let event = headers.get("X-GitHub-Event").unwrap();
    if event != "workflow_job" {
        return build_response(StatusCode::OK);
    }

    if payload.action != "queued" {
        return build_response(StatusCode::OK);
    }

    let payload = serde_json::from_slice::<PayloadWithJob>(&body).unwrap();

    if !payload
        .workflow_job
        .labels
        .contains(&"self-hosted".to_string())
    {
        return build_response(StatusCode::OK);
    }

    if payload.workflow_job.labels.len() != 2 {
        return build_response(StatusCode::OK);
    }

    let config_label = payload
        .workflow_job
        .labels
        .iter()
        .find(|label| label.starts_with("aws-ecs-"));
    let config_label = if let Some(label) = config_label {
        label
    } else {
        return build_response(StatusCode::OK);
    };

    let (cpu, memory, timeout) = match config_label.as_ref() {
        "aws-ecs-0.25cpu-0.5mem-30m" => (256, 512, "30m"),
        "aws-ecs-16cpu-64mem-30m" => (16384, 65536, "30m"),
        _ => {
            info!("invalid config label: {config_label}");
            return build_response(StatusCode::OK);
        }
    };

    info!("handling webhook: {payload:?}");

    let pat = std::env::var("GITHUB_PAT").unwrap();
    let org = payload.repository.owner.login;
    let repo = payload.repository.name;

    let token = get_runner_registration_token(&pat, &org, &repo).await;
    spawn_runner(
        &token,
        &org,
        &repo,
        payload.workflow_job.labels,
        cpu,
        memory,
        timeout,
    )
    .await;

    build_response(StatusCode::OK)
}
