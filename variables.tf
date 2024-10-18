variable "webhook_runners_github_app_id" {
  type = number
  description = "The ID of the GitHub App"
}

variable "webhook_runners_github_app_private_key" {
  type = string
  description = "The private key for the GitHub App. Should be in PEM format."
}

variable "webhook_runners_github_app_installation_id" {
  type = string
  description = "The installation ID of the GitHub App"
}

variable "webhook_runners_github_webhook_secret" {
  type = string
}
