variable "github_app_id" {
  type = number
  description = "The ID of the GitHub App"
}

variable "github_app_private_key" {
  type = string
  description = "The private key for the GitHub App. Should be in PEM format."
}

variable "github_installations" {
  type        = map(number)
  description = "Map of GitHub organization names to their GitHub App installation IDs"
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
