use aws_config::{BehaviorVersion, Region};
use aws_sdk_ecs::types::{
    AwsVpcConfiguration, ContainerOverride, KeyValuePair, NetworkConfiguration, TaskOverride,
};
use axum::{
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use hyper::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new().route("/v1/webhook", post(handle_webhook));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Serialize, Deserialize)]
struct Payload {
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

async fn handle_webhook(headers: HeaderMap, Json(payload): Json<Payload>) -> Response {
    // https://docs.github.com/en/webhooks/using-webhooks/validating-webhook-deliveries
    let _signature = headers.get("X-Hub-Signature-256").unwrap();
    // TODO validate signature

    let event = headers.get("X-GitHub-Event").unwrap();
    if event != "workflow_job" {
        return StatusCode::OK.into_response();
    }

    if payload.action != "queued" {
        return StatusCode::OK.into_response();
    }

    if !payload
        .workflow_job
        .labels
        .contains(&"self-hosted".to_string())
        && !payload
            .workflow_job
            .labels
            .contains(&"playwright-chris-test".to_string())
    {
        return StatusCode::OK.into_response();
    }

    info!("handling webhook: {payload:?}");

    let pat = std::env::var("GITHUB_PAT").unwrap();
    let org = payload.repository.owner.login;
    let repo = payload.repository.name;
    spawn_runner(&pat, &org, &repo, payload.workflow_job.labels).await;

    StatusCode::OK.into_response()
}

async fn spawn_runner(github_pat: &str, org: &str, repo: &str, labels: Vec<String>) {
    let token = get_runner_registration_token(github_pat, org, repo).await;
    run_task(&token, org, repo, labels).await;
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

async fn run_task(runner_token: &str, org: &str, repo: &str, labels: Vec<String>) {
    if labels.is_empty() {
        panic!("labels must not be empty");
    }

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

    // Customizable by repo. TODO parse from label
    let cpu = 16384;
    let memory = 65536;
    let labels = labels.join(",");
    let timeout = "30m";

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
                                .value(format!("aws-ecs-fargate-{cpu}cpu-{memory}mem"))
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
