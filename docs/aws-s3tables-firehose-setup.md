# AWS S3 Tables + Firehose Setup

This document explains how to deploy otlp2pipeline with AWS S3 Tables and Firehose.

## Quick Start

```bash
# 1. Generate CloudFormation template
otlp2pipeline aws create --output template.yaml

# 2. Deploy everything (idempotent - safe to re-run)
./scripts/aws-deploy.sh template.yaml --env prod --region us-east-1

# 3. Check status
./scripts/aws-deploy.sh status --env prod --region us-east-1

# 4. Test
./scripts/aws-send-test-record.sh otlp2pipeline-prod us-east-1
```

The `--env` flag derives consistent names: `--env prod` creates stack `otlp2pipeline-prod` and bucket `otlp2pipeline-prod`.

## What the Deploy Script Does

The `aws-deploy.sh` script handles the complete deployment:

1. **S3 Tables Setup**
   - Creates/updates `S3TablesRoleForLakeFormation` IAM role
   - Adds caller as Lake Formation Data Lake Administrator
   - Registers S3 Tables resource with Lake Formation
   - Creates `s3tablescatalog` federated catalog in Glue
   - Grants catalog permissions to caller

2. **CloudFormation Stack**
   - S3 Table Bucket
   - Namespace
   - Iceberg Tables with OTLP schemas (logs, traces, sum, gauge)
   - Firehose IAM Role
   - Error bucket and logging

3. **LakeFormation Permissions**
   - Grants DESCRIBE on database (namespace) to Firehose role
   - Grants ALL on each table to Firehose role

4. **Firehose Streams (via API)**
   - Creates Firehose delivery streams with **AppendOnly mode** enabled
   - AppendOnly mode enables auto-scaling with no throughput limit
   - Streams are created via AWS API (not CloudFormation) because:
     - CloudFormation doesn't support the `AppendOnly` flag
     - Default throughput is limited to 5 MiB/s without AppendOnly

The script is idempotent - it skips steps that are already complete.

## Tear Down

```bash
./scripts/aws-deploy.sh destroy --env prod --region us-east-1 --force
```

The destroy command:
1. Deletes Firehose streams first (they depend on the IAM role)
2. Deletes the CloudFormation stack

Note: Global resources (IAM role, Glue catalog, LakeFormation config) are preserved as they may be shared across stacks.

## AppendOnly Mode

By default, Firehose to Iceberg has throughput limits:
- **Without AppendOnly**: 5 MiB/s in major regions, 1 MiB/s elsewhere
- **With AppendOnly**: Auto-scaling with no throughput limit

The trade-off is that AppendOnly mode:
- Disables upsert/deduplication (no `UniqueKeys`)
- All records are appended (hence the name)

For OTLP telemetry data, append-only is the correct semantic since we don't need deduplication.

## Background

S3 Tables is AWS's Iceberg-native table format for S3. It integrates with Lake Formation for access control, but there are several limitations that require a hybrid deployment approach.

### CloudFormation Limitations

1. **`AWS::LakeFormation::PrincipalPermissions` doesn't support s3tablescatalog**
   - The CloudFormation resource only accepts AWS account IDs as `CatalogId`
   - S3 Tables requires the format `account-id:s3tablescatalog/bucket-name`
   - This is a known AWS bug: https://github.com/aws/aws-cli/issues/9618

2. **Firehose AppendOnly not supported in CloudFormation**
   - The `AppendOnly` flag is only available via `CreateDeliveryStream` API
   - CloudFormation's `AWS::KinesisFirehose::DeliveryStream` doesn't expose this option

3. **Chicken-and-egg problem**
   - Firehose role needs LakeFormation permissions before Firehose can be created
   - But the role is created as part of the CloudFormation stack
   - No way to grant permissions mid-stack without a custom resource

## Testing

Send a test record to Firehose:

```bash
./scripts/aws-send-test-record.sh otlp2pipeline-prod us-east-1
```

To verify delivery succeeded (wait ~60 seconds for Firehose buffering):

```bash
aws cloudwatch get-metric-statistics \
  --namespace AWS/Firehose \
  --metric-name DeliveryToIceberg.SuccessfulRowCount \
  --dimensions Name=DeliveryStreamName,Value=otlp2pipeline-prod-logs \
  --start-time $(date -u -v-5M '+%Y-%m-%dT%H:%M:%SZ') \
  --end-time $(date -u '+%Y-%m-%dT%H:%M:%SZ') \
  --period 60 --statistics Sum --region us-east-1
```

## Troubleshooting

### "Insufficient Glue permissions to access database"

This error occurs when:
1. The S3 Tables integration isn't properly set up
2. The caller isn't a LakeFormation admin
3. The caller doesn't have permissions on the s3tablescatalog

Fix: Re-run `./scripts/aws-deploy.sh` - it will update all permissions.

### "Unable to assume role" / "Unable to retrieve credentials from Lake Formation"

Lake Formation can't assume the `S3TablesRoleForLakeFormation` role. Common causes:

1. **Missing STS actions in trust policy** (most common)
   - The trust policy must include `sts:AssumeRole`, `sts:SetSourceIdentity`, AND `sts:SetContext`
   - See [S3 Tables + Lake Formation Prerequisites](https://docs.aws.amazon.com/lake-formation/latest/dg/s3tables-catalog-prerequisites.html)

2. **Missing s3tables permissions**
   - The role needs all `s3tables:*` permissions (ListTableBuckets, GetTable, GetTableData, etc.)

3. **IAM eventual consistency**
   - Wait 10-15 seconds after role creation/update

4. **Role not registered with Lake Formation**
   - The resource must be registered with `--with-federation`

**Fix:** Re-run `./scripts/aws-deploy.sh` - it will update the existing role with correct policies.

### "Catalog not found"

The `s3tablescatalog` federated catalog doesn't exist. Re-run the deploy script.

### Firehose stream shows "AppendOnly: false"

If you deployed before this update, your Firehose streams were created via CloudFormation without AppendOnly mode. To upgrade:

1. Delete the existing streams: `./scripts/aws-deploy.sh destroy --env prod --force`
2. Redeploy: `./scripts/aws-deploy.sh template.yaml --env prod`

Or manually delete just the Firehose streams and re-run deploy (it will recreate them with AppendOnly).

## AWS Resources

- [S3 Tables User Guide](https://docs.aws.amazon.com/AmazonS3/latest/userguide/s3-tables.html)
- [Granting LakeFormation Permissions to S3 Tables](https://docs.aws.amazon.com/AmazonS3/latest/userguide/grant-permissions-tables.html)
- [S3 Tables + Lake Formation Prerequisites](https://docs.aws.amazon.com/lake-formation/latest/dg/s3tables-catalog-prerequisites.html)
- [Firehose Iceberg Destination](https://docs.aws.amazon.com/firehose/latest/dev/apache-iceberg-destination.html)

## Known AWS Bugs

- [aws-cli #9618](https://github.com/aws/aws-cli/issues/9618): `grant-permissions --catalog-id` doesn't accept s3tablescatalog format
- [terraform-provider-aws #40724](https://github.com/hashicorp/terraform-provider-aws/issues/40724): LakeFormation permissions for S3Tables Catalog validation error
