use aws_config::{BehaviorVersion, Region};
use aws_sdk_ecs::types::{
    AwsVpcConfiguration, ContainerOverride, EphemeralStorage, KeyValuePair, NetworkConfiguration,
    Tag, TaskOverride,
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
    repository: String,
    job_url: String,
) -> Result<(), anyhow::Error> {
    if labels.is_empty() {
        return Err(anyhow::anyhow!("labels must not be empty"));
    }
    let labels = labels.join(",");

    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new("eu-central-1"))
        .load()
        .await;
    let client = aws_sdk_ecs::Client::new(&config);

    // TF config
    let cluster_arn =
        &std::env::var("CLUSTER_ARN").map_err(|e| anyhow::anyhow!("missing CLUSTER_ARN: {e}"))?;
    let task_definition = "github-actions-runner";
    let subnet =
        &std::env::var("SUBNET_ID").map_err(|e| anyhow::anyhow!("missing SUBNET_ID: {e}"))?;

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
        .tags(Tag::builder().key("Repository").value(repository).build())
        .tags(Tag::builder().key("Size").value(format!("{cpu}cpu-{memory}mem-{disk}disk-{timeout}")).build())
        .tags(Tag::builder().key("Job").value(job_url).build())
        .network_configuration(
            NetworkConfiguration::builder()
                .awsvpc_configuration(
                    AwsVpcConfiguration::builder()
                        .subnets(subnet)
                        .build()
                        .map_err(|e| {
                            anyhow::anyhow!("failed to build network configuration: {e}")
                        })?,
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
        .map_err(|e| anyhow::anyhow!("failed to spawn runner: {e:?}"))?;

    for failure in result.failures() {
        info!("failure: {failure:?}");
    }

    info!("spawned runner: {result:?}");
    Ok(())
}
