variable "webhook_runners_github_pat" {
  type = string
  description = "A GitHub PAT that's able to register runners on the repository. Should be admin access to the repo as described here:https://docs.github.com/en/rest/actions/self-hosted-runners?apiVersion=2022-11-28#create-a-registration-token-for-a-repository"
}

variable "webhook_runners_github_webhook_secret" {
  type = string
}
