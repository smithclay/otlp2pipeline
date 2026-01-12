# Plan: Migrate aws-deploy.sh to Native Rust CLI

## Overview

Replace the 1270-line bash script (`scripts/aws-deploy.sh`) with native Rust CLI commands. The `aws create` command will perform full deployment (like Cloudflare's `create`), eliminating the need for the bash script.

## Design Decisions

| Decision | Choice |
|----------|--------|
| AWS interaction | Shell out to AWS CLI (not SDK) |
| Commands to migrate | All: create (deploy), status, destroy, plan |
| Idempotency | Full - can re-run at any point |
| Code organization | Submodules per AWS service |
| CLI wrapper | Shared `AwsCli` struct with typed methods |
| CloudFormation | Generate dynamically (not embedded) |
| Local Lambda | Supported via `--local` flag |
| Dry-run | Via `plan` command |

## Architecture

```
src/cli/commands/aws/
├── mod.rs              # Re-exports
├── create.rs           # Full deployment orchestration
├── status.rs           # Enhanced status (LakeFormation, partitions, Firehose)
├── destroy.rs          # Complete teardown
├── plan.rs             # Dry-run showing what would happen
├── helpers.rs          # Config loading, naming, validation
├── cfn.rs              # Dynamic CloudFormation template generation
└── cli/                # AWS CLI wrappers by service
    ├── mod.rs          # AwsCli struct with region context
    ├── iam.rs          # create_role, put_role_policy, get_role
    ├── lakeformation.rs# put_data_lake_settings, register_resource, grant_permissions
    ├── glue.rs         # create_catalog, delete_catalog, get_catalog
    ├── cloudformation.rs# deploy, describe_stacks, delete_stack, wait
    ├── athena.rs       # start_query_execution, get_query_execution
    ├── firehose.rs     # create_delivery_stream, describe, delete
    ├── s3tables.rs     # delete_table
    ├── s3.rs           # rm (bucket cleanup)
    ├── lambda.rs       # create/update function, get_function_url_config
    └── sts.rs          # get_caller_identity (account_id, caller_arn)
```

## Implementation Tasks

### 1. Create AwsCli Foundation (`cli/mod.rs`, `cli/sts.rs`)

Create the core `AwsCli` struct that holds region and provides service accessors:

```rust
pub struct AwsCli {
    region: String,
}

impl AwsCli {
    pub fn new(region: &str) -> Self;
    pub fn region(&self) -> &str;

    // Service accessors
    pub fn sts(&self) -> StsCli<'_>;
    pub fn iam(&self) -> IamCli<'_>;
    pub fn lakeformation(&self) -> LakeFormationCli<'_>;
    pub fn glue(&self) -> GlueCli<'_>;
    pub fn cloudformation(&self) -> CloudFormationCli<'_>;
    pub fn athena(&self) -> AthenaCli<'_>;
    pub fn firehose(&self) -> FirehoseCli<'_>;
    pub fn s3tables(&self) -> S3TablesCli<'_>;
    pub fn s3(&self) -> S3Cli<'_>;
    pub fn lambda(&self) -> LambdaCli<'_>;
}

// Shared execution helpers
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub already_existed: bool,
}

/// Run command, treating specific error patterns as "already exists"
pub fn run_idempotent(cmd: &mut Command, expected_error: Option<&str>) -> Result<CommandOutput>;

/// Run command and parse JSON output
pub fn run_json<T: DeserializeOwned>(cmd: &mut Command) -> Result<T>;
```

Implement `StsCli` for getting account info:
```rust
pub struct AccountInfo {
    pub account_id: String,
    pub caller_arn: String,
}

impl StsCli<'_> {
    pub fn get_caller_identity(&self) -> Result<AccountInfo>;
}
```

### 2. Implement IAM CLI Wrapper (`cli/iam.rs`)

```rust
impl IamCli<'_> {
    /// Create role with trust policy. Returns Ok(true) if created, Ok(false) if exists.
    pub fn create_role(&self, name: &str, trust_policy: &serde_json::Value) -> Result<bool>;

    /// Update trust policy on existing role
    pub fn update_assume_role_policy(&self, name: &str, trust_policy: &serde_json::Value) -> Result<()>;

    /// Put inline policy (always succeeds - idempotent)
    pub fn put_role_policy(&self, role: &str, policy_name: &str, policy: &serde_json::Value) -> Result<()>;

    /// Check if role exists
    pub fn get_role(&self, name: &str) -> Result<Option<RoleInfo>>;
}
```

### 3. Implement LakeFormation CLI Wrapper (`cli/lakeformation.rs`)

```rust
impl LakeFormationCli<'_> {
    /// Set data lake admins (idempotent)
    pub fn put_data_lake_settings(&self, admins: &[String]) -> Result<()>;

    /// Register S3 Tables resource
    pub fn register_resource(&self, resource_arn: &str, role_arn: &str) -> Result<bool>;

    /// Deregister resource (for re-registration)
    pub fn deregister_resource(&self, resource_arn: &str) -> Result<()>;

    /// Grant permissions on catalog/database/table
    pub fn grant_permissions(&self, grant: &PermissionGrant) -> Result<bool>;

    /// List permissions for a principal
    pub fn list_permissions(&self, principal: &str, resource: &Resource) -> Result<Vec<Permission>>;

    /// Check if resource is registered
    pub fn describe_resource(&self, resource_arn: &str) -> Result<Option<ResourceInfo>>;
}
```

### 4. Implement Glue CLI Wrapper (`cli/glue.rs`)

```rust
impl GlueCli<'_> {
    /// Create federated catalog for S3 Tables
    pub fn create_catalog(&self, name: &str, resource_arn: &str) -> Result<bool>;

    /// Delete catalog
    pub fn delete_catalog(&self, name: &str) -> Result<()>;

    /// Check if catalog exists
    pub fn get_catalog(&self, name: &str) -> Result<Option<CatalogInfo>>;

    /// Get table metadata (for partition checking)
    pub fn get_table(&self, catalog_id: &str, database: &str, table: &str) -> Result<Option<TableInfo>>;
}
```

### 5. Implement CloudFormation CLI Wrapper (`cli/cloudformation.rs`)

```rust
impl CloudFormationCli<'_> {
    /// Deploy stack (create or update)
    pub fn deploy(&self, stack_name: &str, template_body: &str, params: &[(&str, &str)]) -> Result<()>;

    /// Get stack status and outputs
    pub fn describe_stack(&self, stack_name: &str) -> Result<Option<StackInfo>>;

    /// Get stack resources
    pub fn describe_stack_resources(&self, stack_name: &str) -> Result<Vec<StackResource>>;

    /// Delete stack
    pub fn delete_stack(&self, stack_name: &str) -> Result<()>;

    /// Wait for stack operation to complete
    pub fn wait_stack_create_complete(&self, stack_name: &str) -> Result<()>;
    pub fn wait_stack_delete_complete(&self, stack_name: &str) -> Result<()>;
}

pub struct StackInfo {
    pub status: String,
    pub outputs: HashMap<String, String>,
}
```

### 6. Implement Athena CLI Wrapper (`cli/athena.rs`)

```rust
impl AthenaCli<'_> {
    /// Execute query and wait for completion
    pub fn execute_query(&self, query: &str, catalog: &str, output_location: &str) -> Result<QueryResult>;

    /// Start query execution (async)
    pub fn start_query_execution(&self, query: &str, catalog: &str, output_location: &str) -> Result<String>;

    /// Get query execution status
    pub fn get_query_execution(&self, query_id: &str) -> Result<QueryStatus>;

    /// Wait for query to complete (polls get_query_execution)
    pub fn wait_query_complete(&self, query_id: &str, timeout_secs: u64) -> Result<QueryStatus>;
}
```

### 7. Implement Firehose CLI Wrapper (`cli/firehose.rs`)

```rust
impl FirehoseCli<'_> {
    /// Create delivery stream with Iceberg destination (AppendOnly mode)
    pub fn create_delivery_stream(&self, config: &FirehoseStreamConfig) -> Result<bool>;

    /// Get stream info
    pub fn describe_delivery_stream(&self, name: &str) -> Result<Option<StreamInfo>>;

    /// Delete stream
    pub fn delete_delivery_stream(&self, name: &str) -> Result<()>;
}

pub struct FirehoseStreamConfig {
    pub name: String,
    pub role_arn: String,
    pub catalog_arn: String,
    pub database: String,
    pub table: String,
    pub log_group: String,
    pub log_stream: String,
    pub error_bucket: String,
    pub error_prefix: String,
    pub batch_interval_secs: u32,
    pub batch_size_mb: u32,
}
```

### 8. Implement S3 Tables CLI Wrapper (`cli/s3tables.rs`)

```rust
impl S3TablesCli<'_> {
    /// Delete table from namespace
    pub fn delete_table(&self, bucket_arn: &str, namespace: &str, table: &str) -> Result<()>;
}
```

### 9. Implement S3 CLI Wrapper (`cli/s3.rs`)

```rust
impl S3Cli<'_> {
    /// Recursively delete all objects in bucket
    pub fn rm_recursive(&self, bucket: &str) -> Result<()>;

    /// Copy file to S3
    pub fn cp(&self, local_path: &str, s3_uri: &str) -> Result<()>;
}
```

### 10. Implement Lambda CLI Wrapper (`cli/lambda.rs`)

```rust
impl LambdaCli<'_> {
    /// Get function info
    pub fn get_function(&self, name: &str) -> Result<Option<FunctionInfo>>;

    /// Get function URL config
    pub fn get_function_url_config(&self, name: &str) -> Result<Option<String>>;

    /// Create function
    pub fn create_function(&self, config: &LambdaConfig) -> Result<()>;

    /// Update function code
    pub fn update_function_code(&self, name: &str, s3_bucket: &str, s3_key: &str) -> Result<()>;

    /// Create function URL
    pub fn create_function_url_config(&self, name: &str, auth_type: &str) -> Result<bool>;

    /// Add permission for function URL
    pub fn add_permission(&self, name: &str, statement_id: &str, action: &str, principal: &str) -> Result<bool>;
}
```

### 11. Dynamic CloudFormation Generation (`cfn.rs`)

Move template generation into Rust code:

```rust
/// Generate CloudFormation template as YAML string
pub fn generate_template(config: &CfnConfig) -> String;

pub struct CfnConfig {
    pub enable_logs: bool,
    pub enable_traces: bool,
    pub enable_sum: bool,
    pub enable_gauge: bool,
    pub table_bucket_name: String,
    pub namespace_name: String,
    pub skip_lambda: bool,
    // ... other parameters
}
```

The template will be generated from Rust structs serialized to YAML, or built as a string template with interpolation.

### 12. Implement Deploy Context

Shared state passed through deployment:

```rust
pub struct DeployContext<'a> {
    pub cli: &'a AwsCli,
    pub account_id: String,
    pub caller_arn: String,
    pub env_name: String,
    pub stack_name: String,
    pub bucket_name: String,
    pub namespace: String,
    pub region: String,
    pub local_build: bool,
}

impl DeployContext<'_> {
    /// Get stack output value (cached after CFN deploy)
    pub fn get_output(&self, key: &str) -> Result<String>;
}
```

### 13. Rewrite `create.rs` with Full Deployment

```rust
pub fn execute_create(args: CreateArgs) -> Result<()> {
    let config = load_config()?;
    let env_name = resolve_env_name(args.env)?;
    let region = resolve_region(args.region, &config);

    validate_name_lengths(&env_name, &region)?;

    let cli = AwsCli::new(&region);
    let account = cli.sts().get_caller_identity()?;

    eprintln!("==> Deploying otlp2pipeline to AWS");
    eprintln!("    Account:   {}", account.account_id);
    eprintln!("    Region:    {}", region);
    eprintln!("    Stack:     {}", stack_name(&env_name));

    let ctx = DeployContext::new(&cli, &account, &env_name, &args);

    // S3 Tables + LakeFormation setup
    setup_s3tables_role(&ctx)?;
    setup_lakeformation_admin(&ctx)?;
    register_s3tables_resource(&ctx)?;
    setup_glue_catalog(&ctx)?;

    // CloudFormation deployment
    let template = cfn::generate_template(&ctx.cfn_config());
    deploy_cloudformation(&ctx, &template)?;

    // Create tables via Athena DDL (with partitions)
    create_tables_via_athena(&ctx)?;

    // Grant LakeFormation permissions to Firehose role
    grant_firehose_permissions(&ctx)?;

    // Create Firehose streams (AppendOnly mode)
    create_firehose_streams(&ctx)?;

    // Optional: Local Lambda build and deploy
    if args.local {
        build_and_deploy_lambda(&ctx)?;
    }

    print_success(&ctx);
    Ok(())
}
```

### 14. Enhance `status.rs`

Add comprehensive status checks from bash script:
- IAM role existence
- LakeFormation admin status
- LakeFormation resource registration
- Glue catalog status
- Stack status and resources
- LakeFormation grants per table
- Table partition specs
- Firehose stream status (AppendOnly check)
- Lambda function URL

### 15. Enhance `destroy.rs`

Complete teardown in correct order:
1. Delete Firehose streams
2. Delete tables from namespace
3. Empty S3 buckets (error bucket, artifact bucket)
4. Delete CloudFormation stack
5. Print note about global resources not deleted

### 16. Implement `plan.rs`

Dry-run that shows what would happen:
- Check current state of each resource
- Print what would be created/updated/skipped
- No mutations

### 17. Update CLI Args

Add new flags to `CreateArgs`:
```rust
/// Build and deploy Lambda from local repo
#[arg(long)]
pub local: bool,
```

### 18. Delete Bash Script and Update Documentation

- Delete `scripts/aws-deploy.sh`
- Update `CLAUDE.md` with new CLI commands
- Update any README references

## Task Checklist

- [ ] Create `cli/mod.rs` with AwsCli struct and helpers
- [ ] Create `cli/sts.rs` with get_caller_identity
- [ ] Create `cli/iam.rs` with role operations
- [ ] Create `cli/lakeformation.rs` with permissions
- [ ] Create `cli/glue.rs` with catalog operations
- [ ] Create `cli/cloudformation.rs` with stack operations
- [ ] Create `cli/athena.rs` with query execution
- [ ] Create `cli/firehose.rs` with stream operations
- [ ] Create `cli/s3tables.rs` with table deletion
- [ ] Create `cli/s3.rs` with bucket operations
- [ ] Create `cli/lambda.rs` with function operations
- [ ] Create `cfn.rs` for dynamic template generation
- [ ] Rewrite `create.rs` with full deployment
- [ ] Enhance `status.rs` with comprehensive checks
- [ ] Enhance `destroy.rs` with complete teardown
- [ ] Implement `plan.rs` for dry-run
- [ ] Add `--local` flag to CreateArgs
- [ ] Delete `scripts/aws-deploy.sh`
- [ ] Update documentation

## Verification

After implementation:
1. `otlp2pipeline aws plan --env test` shows deployment plan
2. `otlp2pipeline aws create --env test` deploys everything
3. `otlp2pipeline aws status --env test` shows comprehensive status
4. `otlp2pipeline aws create --env test` is idempotent (no changes)
5. `otlp2pipeline aws create --env test --local` builds and deploys Lambda
6. `otlp2pipeline aws destroy --env test --force` cleans up everything
