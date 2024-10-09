use aws_config::{BehaviorVersion, Region};
use aws_sdk_ecs::types::{
    AwsVpcConfiguration, ContainerOverride, KeyValuePair, NetworkConfiguration, TaskOverride,
};
use serde::{Deserialize, Serialize};
// use axum::Router;

// #[tokio::main]
// async fn main() {
//     // initialize tracing
//     tracing_subscriber::fmt::init();

//     // build our application with a route
//     let app = Router::new()
//         // `GET /` goes to `root`
//         .route("/", get(root))
//         // `POST /users` goes to `create_user`
//         .route("/users", post(create_user));

//     // run our app with hyper, listening globally on port 3000
//     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
//     axum::serve(listener, app).await.unwrap();
// }

#[tokio::main]
async fn main() {
    let pat = std::env::var("GITHUB_PAT").unwrap();
    let org = "reown-com";
    let repo = "appkit";
    spawn_runner(&pat, org, repo).await;
}

async fn spawn_runner(github_pat: &str, org: &str, repo: &str) {
    let token = get_runner_registration_token(github_pat, org, repo).await;
    run_task(&token, org, repo).await;
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

async fn run_task(runner_token: &str, org: &str, repo: &str) {
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
    let labels = "playwright-chris-test"; // TODO get from webhook
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
    println!("result: {result:?}");
}
