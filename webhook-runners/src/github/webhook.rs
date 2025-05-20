use crate::{
    build_response, ecs::spawn_runner, github::runner_registration::get_runner_registration_token,
};
use constant_time_eq::constant_time_eq;
use hmac::{Hmac, Mac};
use http::{HeaderMap, StatusCode};
use lambda_http::{Body, Error, Response};
use octocrab::models::InstallationId;
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

    let (cpu, memory, disk, timeout) = match config_label.as_ref() {
        "aws-ecs-0.25cpu-0.5mem-30m" => (256, 512, 20, "30m"),
        "aws-ecs-16cpu-64mem-30m" => (16384, 65536, 20, "30m"),
        "aws-ecs-16cpu-64mem-20disk-30m" => (16384, 65536, 20, "30m"),
        "aws-ecs-16cpu-64mem-30disk-30m" => (16384, 65536, 30, "30m"),
        "aws-ecs-16cpu-64mem-40disk-30m" => (16384, 65536, 40, "30m"),
        "aws-ecs-16cpu-32mem-20disk-30m" => (16384, 32768, 20, "30m"),
        "aws-ecs-16cpu-24mem-20disk-30m" => (16384, 24576, 20, "30m"),
        "aws-ecs-16cpu-16mem-20disk-30m" => (16384, 16384, 20, "30m"),
        "aws-ecs-12cpu-8mem-20disk-30m" => (12288, 8192, 20, "30m"),
        "aws-ecs-8cpu-8mem-20disk-30m" => (8192, 8192, 20, "30m"),
        _ => {
            info!("invalid config label: {config_label}");
            return build_response(StatusCode::OK);
        }
    };

    info!("handling webhook: {payload:?}");

    // TODO support multiple installations for different orgs
    let installation_id = InstallationId::from(
        std::env::var("GITHUB_APP_INSTALLATION_ID")
            .unwrap()
            .parse::<u64>()
            .unwrap(),
    );
    let org = payload.repository.owner.login;

    let token = get_runner_registration_token(installation_id, &org).await;
    spawn_runner(
        &token,
        &org,
        payload.workflow_job.labels,
        cpu,
        memory,
        disk,
        timeout,
    )
    .await;

    build_response(StatusCode::OK)
}
