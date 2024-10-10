module "lambda" {
  source  = "terraform-aws-modules/lambda/aws"
  version = "7.8.1"

  function_name = "webhook-runners"
  description   = "Function to spawn ECS runners from GitHub webhooks"
  handler       = "bootstrap"
  runtime       = "provided.al2023"
  timeout       = 10
  architectures = ["arm64"]

  tracing_mode = "Active"

  create_package             = false
  publish                    = true
  local_existing_package     = "./webhook-runners/target/lambda/webhook-runners/bootstrap.zip"
  create_lambda_function_url = true

  environment_variables = {
    RUST_BACKTRACE        = 1,
    GITHUB_WEBHOOK_SECRET = var.github_webhook_secret,
    GITHUB_PAT            = var.github_pat,
    CLUSTER_ARN           = var.cluster_arn,
    SUBNET_ID             = var.subnet_id,
  }

  attach_policy_statements = true
  policy_statements = {
    ecs_run_task = {
      effect    = "Allow"
      actions   = ["ecs:RunTask"]
      Condition = {
        "ArnEquals": {
          "ecs:cluster": var.cluster_arn
        }
      }
      resources = [var.task_definition_arn]
    }
    pass_role = {
      effect    = "Allow"
      actions   = ["iam:PassRole"]
      resources = [var.iam_role_arn]
    }
  }
}
