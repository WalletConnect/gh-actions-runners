variable "github_app_id" {
  type = number
  description = "The ID of the GitHub App"
}

variable "github_app_private_key" {
  type = string
  description = "The private key for the GitHub App. Should be in PEM format."
}

variable "github_app_installation_id" {
  type = string
  description = "The installation ID of the GitHub App"
}

variable "github_webhook_secret" {
  type = string
}

variable "cluster_arn" {
  type = string
}

variable "task_definition_arn" {
  type = string
}

variable "subnet_id" {
  type = string
}

variable "iam_role_arn" {
  type = string
}
