variable "cpu" {
  type = number
}

variable "memory" {
  type = number
}

variable "runner_token" {
  type = string
}

variable "repo_url" {
  type = string
}

variable "labels" {
  type = string
}

variable "desired_count" {
  type = number
}

variable "timeout" {
  type = string
}

data "aws_ecs_cluster" "this" {
  cluster_name = "github-actions-runner"
}

data "aws_subnet" "this" {
  cidr_block = "10.0.2.0/24"
  filter {
    name   = "tag:Application"
    values = ["github-actions-runners"]
  }
}

data "aws_ecs_task_execution" "this" {
  cluster         = data.aws_ecs_cluster.this.id
  task_definition = "github-actions-runner"
  desired_count   = var.desired_count

  network_configuration {
    subnets = [data.aws_subnet.this.id]
  }

  overrides {
    cpu    = var.cpu
    memory = var.memory

    container_overrides {
      name   = "github-actions-runner"
      cpu    = var.cpu
      memory = var.memory

      environment {
        key   = "RUNNER_NAME_PREFIX"
        value = "aws-ecs-fargate-${var.cpu}cpu-${var.memory}mem"
      }
      environment {
        key   = "RUNNER_TOKEN"
        value = var.runner_token
      }
      environment {
        key   = "REPO_URL"
        value = var.repo_url
      }
      environment {
        key   = "LABELS"
        value = var.labels
      }
      environment {
        key   = "EPHEMERAL"
        value = true
      }
      environment {
        key   = "START_DOCKER_SERVICE"
        value = true
      }
      environment {
        key   = "TIMEOUT"
        value = var.timeout
      }
    }
  }
}

