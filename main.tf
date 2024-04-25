terraform {
  required_version = ">= 1.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = ">= 5.0"
    }
  }

  backend "remote" {
    hostname     = "app.terraform.io"
    organization = "wallet-connect"
    workspaces {
      name = "github-actions-runners"
    }
  }
}

provider "aws" {
  region = "eu-central-1"

  default_tags {
    tags = {
      Application = "github-actions-runners"
    }
  }
}

variable "run_task" {
  type    = bool
  default = false
}

variable "cpu" {
  type    = number
  default = 256
}

variable "memory" {
  type    = number
  default = 512
}

variable "runner_token" {
  type    = string
  default = null
}

variable "repo_url" {
  type    = string
  default = null
}

variable "labels" {
  type    = string
  default = null
}

variable "desired_count" {
  type    = number
  default = null
}

variable "timeout" {
  type    = string
  default = null
}

locals {
  availability_zone = "eu-central-1a"
}

resource "aws_vpc" "this" {
  cidr_block = "10.0.0.0/16"

  # Required for EFS
  enable_dns_hostnames = true
}

resource "aws_subnet" "public" {
  vpc_id            = aws_vpc.this.id
  cidr_block        = "10.0.1.0/24"
  availability_zone = local.availability_zone
}

resource "aws_route_table" "public" {
  vpc_id = aws_vpc.this.id
}

resource "aws_internet_gateway" "this" {
  vpc_id = aws_vpc.this.id
}

resource "aws_route" "internet_gateway" {
  route_table_id         = aws_route_table.public.id
  destination_cidr_block = "0.0.0.0/0"
  gateway_id             = aws_internet_gateway.this.id
}

resource "aws_route_table_association" "public" {
  subnet_id      = aws_subnet.public.id
  route_table_id = aws_route_table.public.id
}

resource "aws_eip" "this" {}

resource "aws_nat_gateway" "this" {
  allocation_id = aws_eip.this.id
  subnet_id     = aws_subnet.public.id

  depends_on = [aws_internet_gateway.this]
}

module "cache-server" {
  source            = "./cache-server"
  vpc_id            = aws_vpc.this.id
  availability_zone = local.availability_zone
  nat_gateway_id    = aws_nat_gateway.this.id
}

module "runner" {
  source            = "./runner"
  vpc_id            = aws_vpc.this.id
  availability_zone = local.availability_zone
  nat_gateway_id    = aws_nat_gateway.this.id
  cache_url         = module.cache-server.url
}

module "setup-runners" {
  count         = var.run_task ? 1 : 0
  source        = "./setup-runners"
  cpu           = var.cpu
  memory        = var.memory
  runner_token  = var.runner_token
  repo_url      = var.repo_url
  labels        = var.labels
  desired_count = var.desired_count
  timeout = var.timeout
}
