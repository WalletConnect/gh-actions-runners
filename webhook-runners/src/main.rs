use aws_config::{BehaviorVersion, Region};
use aws_sdk_ecs::types::{
    AwsVpcConfiguration, ContainerOverride, KeyValuePair, NetworkConfiguration, TaskOverride,
};
use constant_time_eq::constant_time_eq;
use hmac::{Hmac, Mac};
use http::HeaderMap;
use lambda_http::{
    http::{Response, StatusCode},
    run, service_fn, Body, Error, Request,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Sha256;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .without_time()
        .with_max_level(tracing::Level::INFO)
        .init();

    run(service_fn(function_handler)).await
}

fn build_response(status: StatusCode) -> Result<Response<String>, Error> {
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

    handle_webhook(headers, body).await
}

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

async fn handle_webhook(headers: HeaderMap, body: Body) -> Result<Response<String>, Error> {
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
    spawn_runner(
        &pat,
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

async fn spawn_runner(
    github_pat: &str,
    org: &str,
    repo: &str,
    labels: Vec<String>,
    cpu: i32,
    memory: i32,
    timeout: &str,
) {
    let token = get_runner_registration_token(github_pat, org, repo).await;
    run_task(&token, org, repo, labels, cpu, memory, timeout).await;
}

#[derive(Debug, Serialize, Deserialize)]
struct RegistrationTokenResponse {
    token: String,
    expires_at: String,
}

async fn get_runner_registration_token(github_pat: &str, org: &str, repo: &str) -> String {
    let response = reqwest::Client::new()
        .post(format!(
            "https://api.github.com/repos/{org}/{repo}/actions/runners/registration-token"
        ))
        .bearer_auth(github_pat)
        .header("Accept", "application/vnd.github.v3+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header(
            "User-Agent",
            "https://github.com/WalletConnect/gh-actions-runners",
        )
        .send()
        .await
        .unwrap();
    if !response.status().is_success() {
        panic!(
            "get_runner_registration_token error response: {:?}",
            response.text().await
        );
    }
    response
        .json::<RegistrationTokenResponse>()
        .await
        .unwrap()
        .token
}

async fn run_task(
    runner_token: &str,
    org: &str,
    repo: &str,
    labels: Vec<String>,
    cpu: i32,
    memory: i32,
    timeout: &str,
) {
    if labels.is_empty() {
        panic!("labels must not be empty");
    }
    let labels = labels.join(",");

    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new("eu-central-1"))
        .load()
        .await;
    let client = aws_sdk_ecs::Client::new(&config);

    // TF config
    let cluster_arn = &std::env::var("CLUSTER_ARN").unwrap();
    let task_definition = "github-actions-runner";
    let subnet = &std::env::var("SUBNET_ID").unwrap();

    // Auto-generated
    let repo_url = format!("https://github.com/{org}/{repo}");

    let result = client
        .run_task()
        .cluster(cluster_arn)
        .task_definition(task_definition)
        .network_configuration(
            NetworkConfiguration::builder()
                .awsvpc_configuration(
                    AwsVpcConfiguration::builder()
                        .subnets(subnet)
                        .build()
                        .unwrap(),
                )
                .build(),
        )
        .overrides(
            TaskOverride::builder()
                .cpu(cpu.to_string())
                .memory(memory.to_string())
                .container_overrides(
                    ContainerOverride::builder()
                        .name("github-actions-runner")
                        .cpu(cpu)
                        .memory(memory)
                        .environment(
                            KeyValuePair::builder()
                                .name("RUNNER_NAME_PREFIX")
                                .value(format!("aws-ecs-fargate-{cpu}cpu-{memory}mem-{timeout}"))
                                .build(),
                        )
                        .environment(
                            KeyValuePair::builder()
                                .name("RUNNER_TOKEN")
                                .value(runner_token)
                                .build(),
                        )
                        .environment(
                            KeyValuePair::builder()
                                .name("REPO_URL")
                                .value(repo_url)
                                .build(),
                        )
                        .environment(KeyValuePair::builder().name("LABELS").value(labels).build())
                        .environment(
                            KeyValuePair::builder()
                                .name("EPHEMERAL")
                                .value("true")
                                .build(),
                        )
                        .environment(
                            KeyValuePair::builder()
                                .name("START_DOCKER_SERVICE")
                                .value("true")
                                .build(),
                        )
                        .environment(
                            KeyValuePair::builder()
                                .name("TIMEOUT")
                                .value(timeout)
                                .build(),
                        )
                        .build(),
                )
                .build(),
        )
        .send()
        .await
        .unwrap();
    info!("spawned runner: {result:?}");
}
