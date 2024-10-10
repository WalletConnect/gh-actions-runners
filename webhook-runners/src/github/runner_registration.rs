use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct RegistrationTokenResponse {
    token: String,
    expires_at: String,
}

pub async fn get_runner_registration_token(github_pat: &str, org: &str, repo: &str) -> String {
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
