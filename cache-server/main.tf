variable "vpc_id" {
  type = string
}

variable "availability_zone" {
  type = string
}

variable "nat_gateway_id" {
  type = string
}

locals {
  name = "github-actions-cache-server"

  hostnum     = 4
  efs_hostnum = 5

  ip     = "10.0.3.${local.hostnum}"
  efs_ip = "10.0.3.${local.efs_hostnum}"

  aws_reserved_hostnums = [0, 1, 2, 3, 15] # for '/28' cidr
  hostnums_to_reserve   = sort(tolist(setsubtract(range(16), concat(local.aws_reserved_hostnums, [local.hostnum, local.efs_hostnum]))))

  port         = 3000
  access_token = "cache"
  base_url     = "http://${local.ip}:${local.port}"
}

resource "aws_subnet" "this" {
  vpc_id            = var.vpc_id
  cidr_block        = "10.0.3.0/28"
  availability_zone = var.availability_zone
}

# We are attaching all free IP addresses except one to this network interface, effectively forcing ECS
# to assign the specific IP we want to the task.
resource "aws_network_interface" "ip_reserve" {
  subnet_id   = aws_subnet.this.id
  private_ips = [for hostnum in local.hostnums_to_reserve : cidrhost(aws_subnet.this.cidr_block, hostnum)]

  tags = {
    Name = "${local.name}-ip-reserve"
  }
}

resource "aws_route_table" "this" {
  vpc_id = var.vpc_id
}

resource "aws_route_table_association" "this" {
  subnet_id      = aws_subnet.this.id
  route_table_id = aws_route_table.this.id
}

resource "aws_route" "nat_gateway" {
  route_table_id         = aws_route_table.this.id
  destination_cidr_block = "0.0.0.0/0"
  nat_gateway_id         = var.nat_gateway_id
}

data "aws_iam_policy_document" "assume_role" {
  statement {
    actions = ["sts:AssumeRole"]

    principals {
      type        = "Service"
      identifiers = ["ecs-tasks.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "this" {
  name               = local.name
  assume_role_policy = data.aws_iam_policy_document.assume_role.json
}

resource "aws_iam_role_policy_attachment" "ecs_task_execution_role_policy" {
  role       = aws_iam_role.this.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy"
}

resource "aws_iam_role_policy_attachment" "cloudwatch_write_policy" {
  role       = aws_iam_role.this.name
  policy_arn = "arn:aws:iam::aws:policy/CloudWatchLogsFullAccess"
}

resource "aws_ecs_cluster" "this" {
  name = local.name
}

resource "aws_ecs_cluster_capacity_providers" "this" {
  cluster_name = aws_ecs_cluster.this.name

  capacity_providers = ["FARGATE_SPOT"]

  default_capacity_provider_strategy {
    capacity_provider = "FARGATE_SPOT"
    weight            = 1
  }
}

resource "aws_cloudwatch_log_group" "this" {
  name              = "${local.name}-logs"
  retention_in_days = 1
}

resource "aws_ecs_task_definition" "this" {
  family                   = local.name
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"

  container_definitions = jsonencode([{
    name      = local.name
    image     = "ghcr.io/falcondev-oss/github-actions-cache-server:1"
    essential = true

    environment = [
      { name = "URL_ACCESS_TOKEN", value = local.access_token },
      { name = "BASE_URL", value = local.base_url },
      { name = "NITRO_PORT", value = "3000" },
      { name = "STORAGE_DRIVER", value = "s3" },
      { name = "S3_BUCKET", value = aws_s3_bucket.this.bucket },
      { name = "S3_ACCESS_KEY", value = aws_iam_access_key.this.id },
      { name = "S3_SECRET_KEY", value = aws_iam_access_key.this.secret },
      { name = "S3_ENDPOINT", value = "s3.eu-central-1.amazonaws.com" },
      { name = "S3_REGION", value = "eu-central-1" },
      { name = "S3_PORT", value = "443" },
      { name = "S3_USE_SSL", value = "true" },
    ]

    logConfiguration = {
      logDriver = "awslogs"
      options = {
        awslogs-group         = aws_cloudwatch_log_group.this.name
        awslogs-region        = "eu-central-1"
        awslogs-stream-prefix = "ecs"
      }
    }

    mountPoints = [{
      containerPath = "/data"
      sourceVolume  = "efs"
    }]
  }])

  cpu                = 1024
  memory             = 4096
  execution_role_arn = aws_iam_role.this.arn


  volume {
    name = "efs"
    efs_volume_configuration {
      file_system_id     = aws_efs_file_system.this.id
      transit_encryption = "ENABLED" # required in order to use an access point

      authorization_config {
        access_point_id = aws_efs_access_point.this.id
      }
    }
  }

  runtime_platform {
    operating_system_family = "LINUX"
    cpu_architecture        = "X86_64"
  }
}

resource "aws_ecs_service" "this" {
  name            = local.name
  cluster         = aws_ecs_cluster.this.id
  task_definition = aws_ecs_task_definition.this.arn
  desired_count   = 1

  deployment_maximum_percent         = 100
  deployment_minimum_healthy_percent = 0

  network_configuration {
    subnets = [aws_subnet.this.id]
  }
}

resource "aws_efs_file_system" "this" {
  throughput_mode = "bursting"

  tags = {
    Name = local.name
  }
}

resource "aws_security_group" "efs_mount_point" {
  name   = "${local.name}-efs-mount-point"
  vpc_id = var.vpc_id

  # 2049 port is required by EFS
  ingress {
    from_port   = 2049
    to_port     = 2049
    protocol    = "tcp"
    cidr_blocks = ["10.0.0.0/16"]
  }
}

resource "aws_efs_mount_target" "this" {
  file_system_id  = aws_efs_file_system.this.id
  subnet_id       = aws_subnet.this.id
  ip_address      = local.efs_ip
  security_groups = [aws_security_group.efs_mount_point.id]
}

resource "aws_efs_access_point" "this" {
  file_system_id = aws_efs_file_system.this.id

  # All filesystem calls is going to be accessed through this user.
  posix_user {
    gid = 0
    uid = 0
  }
}

resource "aws_iam_user" "this" {
  name = local.name
}

resource "aws_iam_access_key" "this" {
  user = aws_iam_user.this.name
}

resource "aws_s3_bucket" "this" {
  bucket_prefix = "github-actions-cache"
  force_destroy = "true"
}

resource "aws_s3_bucket_policy" "this" {
  bucket = aws_s3_bucket.this.id

  policy = <<EOF
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Principal": {
        "AWS": "${aws_iam_user.this.arn}"
      },
      "Action": [ "s3:*" ],
      "Resource": [
        "${aws_s3_bucket.this.arn}",
        "${aws_s3_bucket.this.arn}/*"
      ]
    }
  ]
}
EOF
}


resource "aws_s3_bucket_lifecycle_configuration" "example" {
  bucket = aws_s3_bucket.this.id

  rule {
    id = "main"

    filter {}
    expiration {
      days = 7
    }

    status = "Enabled"
  }
}

output "url" {
  value = "${local.base_url}/${local.access_token}/"
}
