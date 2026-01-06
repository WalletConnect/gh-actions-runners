use crate::{
    build_response, ecs::spawn_runner, github::runner_registration::get_runner_registration_token,
};
use anyhow::Context;
use constant_time_eq::constant_time_eq;
use hmac::{Hmac, Mac};
use http::{HeaderMap, StatusCode};
use lambda_http::{Body, Response};
use octocrab::models::InstallationId;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
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
    html_url: String,
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

pub async fn handle_webhook(headers: HeaderMap, body: Body) -> anyhow::Result<Response<String>> {
    // https://docs.github.com/en/webhooks/using-webhooks/validating-webhook-deliveries
    let signature = headers
        .get("X-Hub-Signature-256")
        .ok_or_else(|| anyhow::anyhow!("missing X-Hub-Signature-256 header"))?
        .to_str()?;
    let signature = signature
        .strip_prefix("sha256=")
        .ok_or_else(|| anyhow::anyhow!("invalid signature: missing prefix"))?;
    let signature = hex::decode(signature).map_err(|_| anyhow::anyhow!("invalid signature"))?;
    let github_webhook_secret =
        std::env::var("GITHUB_WEBHOOK_SECRET").context("missing GITHUB_WEBHOOK_SECRET")?;
    let mut mac = Hmac::<Sha256>::new_from_slice(github_webhook_secret.as_bytes())
        .context("failed to create HMAC")?;
    mac.update(&body);
    let result = mac.finalize().into_bytes();
    if !constant_time_eq(&result[..], &signature[..]) {
        return Ok(build_response(StatusCode::UNAUTHORIZED)
            .map_err(|e| anyhow::anyhow!("failed to build response: {e}"))?);
    }

    info!("webhook received: {:?}", String::from_utf8_lossy(&body));

    let payload = serde_json::from_slice::<Payload>(&body)
        .map_err(|e| anyhow::anyhow!("failed to parse payload: {e}"))?;

    let event = headers
        .get("X-GitHub-Event")
        .ok_or_else(|| anyhow::anyhow!("missing X-GitHub-Event header"))?;
    if event != "workflow_job" {
        return Ok(build_response(StatusCode::OK)
            .map_err(|e| anyhow::anyhow!("failed to build response: {e}"))?);
    }

    if payload.action != "queued" {
        return Ok(build_response(StatusCode::OK)
            .map_err(|e| anyhow::anyhow!("failed to build response: {e}"))?);
    }

    let payload = serde_json::from_slice::<PayloadWithJob>(&body)
        .map_err(|e| anyhow::anyhow!("failed to parse payload with job: {e}"))?;

    if !payload
        .workflow_job
        .labels
        .contains(&"self-hosted".to_string())
    {
        return Ok(build_response(StatusCode::OK)
            .map_err(|e| anyhow::anyhow!("failed to build response: {e}"))?);
    }

    if payload.workflow_job.labels.len() != 2 {
        return Ok(build_response(StatusCode::OK)
            .map_err(|e| anyhow::anyhow!("failed to build response: {e}"))?);
    }

    let config_label = payload
        .workflow_job
        .labels
        .iter()
        .find(|label| label.starts_with("aws-ecs-"));
    let config_label = if let Some(label) = config_label {
        label
    } else {
        return Ok(build_response(StatusCode::OK)
            .map_err(|e| anyhow::anyhow!("failed to build response: {e}"))?);
    };

    let (cpu, memory, disk, timeout) = match config_label.as_ref() {
        "aws-ecs-0.25cpu-0.5mem-30m" => (256, 512, 20, 30),
        "aws-ecs-16cpu-64mem-30m" => (16384, 65536, 20, 30),
        "aws-ecs-16cpu-64mem-20disk-30m" => (16384, 65536, 20, 30),
        "aws-ecs-16cpu-64mem-30disk-30m" => (16384, 65536, 30, 30),
        "aws-ecs-16cpu-64mem-40disk-30m" => (16384, 65536, 40, 30),
        "aws-ecs-16cpu-32mem-20disk-30m" => (16384, 32768, 20, 30),
        "aws-ecs-16cpu-24mem-20disk-30m" => (16384, 24576, 20, 30),
        "aws-ecs-16cpu-16mem-20disk-30m" => (16384, 16384, 20, 30),
        "aws-ecs-12cpu-8mem-20disk-30m" => (12288, 8192, 20, 30),
        "aws-ecs-8cpu-8mem-20disk-30m" => (8192, 8192, 20, 30),
        "aws-ecs-4cpu-8mem-20disk-30m" => (4096, 8192, 20, 30),
        c => match parse_config_label(c) {
            Ok(c) => c,
            Err(()) => {
                info!("invalid config label: {config_label}");
                return Ok(build_response(StatusCode::OK)
                    .map_err(|e| anyhow::anyhow!("failed to build response: {e}"))?);
            }
        },
    };

    info!("handling webhook: {payload:?}");

    let org = payload.repository.owner.login;
    let repo = payload.repository.name;
    let job_url = payload.workflow_job.html_url;

    // Look up installation ID for this organization
    let installations_json = std::env::var("GITHUB_INSTALLATIONS")
        .context("missing GITHUB_INSTALLATIONS environment variable")?;
    let installations: HashMap<String, u64> = serde_json::from_str(&installations_json)
        .context("failed to parse GITHUB_INSTALLATIONS as JSON")?;

    let installation_id = installations
        .get(&org)
        .ok_or_else(|| anyhow::anyhow!("no installation ID configured for organization: {org}"))?;
    let installation_id = InstallationId::from(*installation_id);

    let token = get_runner_registration_token(installation_id, &org).await;

    let timeout_str = format!("{}m", timeout);
    spawn_runner(
        &token,
        &org,
        payload.workflow_job.labels,
        cpu as i32,
        memory as i32,
        disk as i32,
        &timeout_str,
        format!("https://github.com/{org}/{repo}/"),
        job_url,
    )
    .await?;

    Ok(build_response(StatusCode::OK)
        .map_err(|e| anyhow::anyhow!("failed to build response: {e}"))?)
}

fn parse_config_label(config_label: &str) -> Result<(u32, u32, u32, u32), ()> {
    // Expected format: aws-ecs-{cpu}cpu-{memory}mem-{disk}disk-{timeout}
    let parts: Vec<&str> = config_label.split('-').collect();
    if parts.len() != 6 || parts.get(0) != Some(&"aws") || parts.get(1) != Some(&"ecs") {
        return Err(());
    }

    let cpu_str = parts.get(2).ok_or(())?.trim_end_matches("cpu");
    let memory_str = parts.get(3).ok_or(())?.trim_end_matches("mem");
    let disk_str = parts.get(4).ok_or(())?.trim_end_matches("disk");
    let timeout_str = parts.get(5).ok_or(())?.trim_end_matches("m");

    let cpu = cpu_str.parse::<u32>().map_err(|_| ())?;
    let memory = memory_str.parse::<u32>().map_err(|_| ())?;
    let disk = disk_str.parse::<u32>().map_err(|_| ())?;
    let timeout = timeout_str.parse::<u32>().map_err(|_| ())?;

    Ok((cpu * 1024, memory * 1024, disk, timeout))
}

#[cfg(test)]
mod tests {
    use {super::*, std::collections::HashMap};

    #[test]
    fn test_parse_config_label_valid() {
        let input = "aws-ecs-16cpu-64mem-20disk-30m";
        let result = parse_config_label(input);
        assert!(result.is_ok());
        let (cpu, memory, disk, timeout) = result.unwrap();
        assert_eq!(cpu, 16 * 1024);
        assert_eq!(memory, 64 * 1024);
        assert_eq!(disk, 20);
        assert_eq!(timeout, 30);
    }

    #[test]
    fn test_parse_config_label_invalid_format() {
        let input = "invalid-format";
        let result = parse_config_label(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_config_label_invalid_numbers() {
        let input = "aws-ecs-abcpu-64mem-20disk-30m";
        let result = parse_config_label(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_config_label_hardcoded_versions() {
        let cases = HashMap::from([
            // ("aws-ecs-0.25cpu-0.5mem-30m", (256, 512, 20, 30)),
            // ("aws-ecs-16cpu-64mem-30m", (16384, 65536, 20, 30)),
            ("aws-ecs-8cpu-8mem-50disk-30m", (8192, 8192, 50, 30)),
            ("aws-ecs-16cpu-64mem-20disk-30m", (16384, 65536, 20, 30)),
            ("aws-ecs-16cpu-64mem-30disk-30m", (16384, 65536, 30, 30)),
            ("aws-ecs-16cpu-64mem-40disk-30m", (16384, 65536, 40, 30)),
            ("aws-ecs-16cpu-32mem-20disk-30m", (16384, 32768, 20, 30)),
            ("aws-ecs-16cpu-24mem-20disk-30m", (16384, 24576, 20, 30)),
            ("aws-ecs-16cpu-16mem-20disk-30m", (16384, 16384, 20, 30)),
            ("aws-ecs-12cpu-8mem-20disk-30m", (12288, 8192, 20, 30)),
            ("aws-ecs-8cpu-8mem-20disk-30m", (8192, 8192, 20, 30)),
            ("aws-ecs-4cpu-8mem-20disk-30m", (4096, 8192, 20, 30)),
        ]);
        for (input, expected) in cases {
            let (cpu, memory, disk, timeout) =
                parse_config_label(input).expect(&format!("failed to parse {input}").as_str());
            assert_eq!(cpu, expected.0);
            assert_eq!(memory, expected.1);
            assert_eq!(disk, expected.2);
            assert_eq!(timeout, expected.3);
        }
    }
}
