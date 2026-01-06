## Deploying webhook runners

```bash
cargo install cargo-lambda
```

```bash
terraform login
terraform init
```

```bash
cd webhook-runners/
cargo lambda build --release --arm64 --output-format zip
```

```bash
terraform plan
terraform apply
```

## Creating GitHub app and webhook

Can be done at an organiation or enterprise level.

- Create the app, give it a name of "Self-hosted GH Actions Runners" and homepage of `https://github.com/WalletConnect/gh-actions-runners`

- Create a webhook:
  - URL: `https://<Lambda URL>/v1/webhook`
  - Secret: generate it and set as `webhook_runners_github_webhook_secret` variable
  - Individual events: Workflow jobs
- Organization permissions:
  - Self-hosted runners: read & write
- Update `webhook_runners_github_app_id` and create a private key and set to `webhook_runners_github_app_private_key`

## Installing GitHub app on the org

Must have `Read and write access to organization self hosted runners` permission

## Usage

```
name: Rust stable - build & lint
runs-on: [self-hosted, aws-ecs-16cpu-64mem-40disk-60m]
```
