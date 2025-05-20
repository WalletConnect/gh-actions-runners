use aws_config::{BehaviorVersion, Region};
use aws_sdk_ecs::types::{
    AwsVpcConfiguration, ContainerOverride, EphemeralStorage, KeyValuePair, NetworkConfiguration,
    TaskOverride,
};
use tracing::info;

pub async fn spawn_runner(
    runner_token: &str,
    org: &str,
    labels: Vec<String>,
    cpu: i32,
    memory: i32,
    disk: i32,
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

    let task_override = TaskOverride::builder()
        .cpu(cpu.to_string())
        .memory(memory.to_string());
    // Docs say min is 20, but it's actually 21
    let task_override = if disk > 20 {
        task_override.ephemeral_storage(EphemeralStorage::builder().size_in_gib(disk).build())
    } else {
        task_override
    };

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
            task_override
                .container_overrides(
                    ContainerOverride::builder()
                        .name("github-actions-runner")
                        .cpu(cpu)
                        .memory(memory)
                        .environment(
                            KeyValuePair::builder()
                                .name("RUNNER_NAME_PREFIX")
                                .value(format!(
                                    "aws-ecs-fargate-{cpu}cpu-{memory}mem-{disk}disk-{timeout}"
                                ))
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
                                .name("RUNNER_SCOPE")
                                .value("org")
                                .build(),
                        )
                        .environment(KeyValuePair::builder().name("ORG_NAME").value(org).build())
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
