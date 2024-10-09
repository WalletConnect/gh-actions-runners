module "lambda" {
  source  = "terraform-aws-modules/lambda/aws"
  version = "7.8.1"

  function_name = "webhook-runners"
  description   = "Function to spawn ECS runners from GitHub webhooks"
  handler       = "bootstrap"
  runtime       = "provided.al2023"
  timeout       = 10
  architectures = ["arm64"]

  environment_variables = {
    RUST_BACKTRACE        = 1,
    GITHUB_WEBHOOK_SECRET = var.github_webhook_secret,
    GITHUB_PAT            = var.github_pat,
    CLUSTER_ARN           = var.cluster_arn,
    SUBNET_ID             = var.subnet_id,
  }

  tracing_mode = "Active"

  #   attach_policies    = true
  #   number_of_policies = 1
  #   policies           = ["arn:aws:iam::898587786287:policy/prod-relay-customer-metrics-data-api-access"]

  create_package             = false
  publish                    = true
  local_existing_package     = "target/lambda/webhook-runners/bootstrap.zip"
  create_lambda_function_url = true
}
