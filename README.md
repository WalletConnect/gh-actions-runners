## Deploying webhook runners

```bash
cargo install cargo-lambda
```

```bash
terraform login
terraform init
```

```bash
cd webhook-runners/
cargo lambda build --release --arm64 --output-format zip
```

```bash
terraform plan
terraform apply
```

## Installing GitHub app on the org

Must have `Read and write access to organization self hosted runners` permission

## Creating webhook on the org

Only needs `Workflow jobs` events
