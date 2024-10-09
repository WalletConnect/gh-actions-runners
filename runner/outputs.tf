output "cluster_arn" {
  value = aws_ecs_cluster.this.arn
}

output "task_definition_arn" {
  value = aws_ecs_task_definition.this.arn
}

output "subnet_id" {
  value = aws_subnet.this.id
}

output "iam_role_arn" {
  value = aws_iam_role.this.arn
}
