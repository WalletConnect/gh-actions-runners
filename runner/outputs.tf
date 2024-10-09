output "cluster_arn" {
  value = aws_ecs_cluster.this.arn
}

output "subnet_id" {
  value = aws_subnet.this.id
}
