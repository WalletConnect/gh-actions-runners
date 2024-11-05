use octocrab::models::InstallationId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct RegistrationTokenResponse {
    token: String,
    expires_at: String,
}

pub async fn get_runner_registration_token(installation_id: InstallationId, org: &str) -> String {
    let response = octocrab::instance()
        .installation(installation_id)
        .unwrap()
        .post::<_, RegistrationTokenResponse>(
            format!("/orgs/{org}/actions/runners/registration-token"),
            None::<&()>,
        )
        .await
        .unwrap();
    response.token
}
