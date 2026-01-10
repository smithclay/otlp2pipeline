# AWS S3 Tables + Firehose Setup

This document explains how to deploy otlp2pipeline with AWS S3 Tables and Firehose.

## Background

S3 Tables is AWS's new Iceberg-native table format for S3. It integrates with Lake Formation for access control, but there are several limitations that require a multi-phase deployment approach.

### CloudFormation Limitations

1. **`AWS::LakeFormation::PrincipalPermissions` doesn't support s3tablescatalog**
   - The CloudFormation resource only accepts AWS account IDs as `CatalogId`
   - S3 Tables requires the format `account-id:s3tablescatalog/bucket-name`
   - This is a known AWS bug: https://github.com/aws/aws-cli/issues/9618

2. **Chicken-and-egg problem**
   - Firehose role needs LakeFormation permissions before Firehose can be created
   - But the role is created as part of the CloudFormation stack
   - No way to grant permissions mid-stack without a custom resource

3. **Lambda custom resource challenges**
   - Lambda needs to be a LakeFormation admin to grant permissions
   - Adding Lambda as admin requires modifying DataLakeSettings
   - Federated catalog access requires complex permission setup
   - Even LakeFormation admins can't grant permissions on s3tablescatalog databases without explicit catalog-level permissions

## Solution: Three-Phase Deployment

### Phase 0: One-time S3 Tables Setup (per account/region)

Run the setup script to enable S3 Tables + Lake Formation integration:

```bash
./scripts/aws-s3tables-setup.sh us-east-1
```

This script:
1. Creates/updates `S3TablesRoleForLakeFormation` IAM role with:
   - Trust policy allowing `sts:AssumeRole`, `sts:SetSourceIdentity`, `sts:SetContext` for Lake Formation
   - Inline policy with all required `s3tables:*` permissions
2. Adds your IAM identity as a Lake Formation Data Lake Administrator
3. Registers S3 Tables resource with Lake Formation (with federation)
4. Creates the `s3tablescatalog` federated catalog in Glue
5. Grants you permissions on the catalog

**Important:** If you encounter "Unable to assume role" errors, re-run this script. It will update
the existing role with the correct trust policy and permissions.

### Phase 1: Deploy Infrastructure

```bash
otlp2pipeline aws create --output template.yaml

aws cloudformation deploy \
  --template-file template.yaml \
  --stack-name otlp2pipeline-prod \
  --region us-east-1 \
  --capabilities CAPABILITY_NAMED_IAM \
  --parameter-overrides Phase=1 TableBucketName=otlp2pipeline NamespaceName=default
```

Phase 1 creates:
- S3 Table Bucket
- Namespace
- Iceberg Table with OTLP logs schema
- Firehose IAM Role (with Glue, S3, CloudWatch permissions)
- Error bucket and logging

### Phase 2: Grant LakeFormation Permissions

After Phase 1 completes, grant LakeFormation permissions to the Firehose role:

```bash
./scripts/aws-grant-firehose-permissions.sh otlp2pipeline-prod us-east-1 otlp2pipeline default
```

This script:
1. Retrieves the Firehose role ARN from CloudFormation stack outputs
2. Grants DESCRIBE on the database (namespace)
3. Grants ALL on the logs table

### Phase 3: Deploy Firehose

```bash
aws cloudformation deploy \
  --template-file template.yaml \
  --stack-name otlp2pipeline-prod \
  --region us-east-1 \
  --capabilities CAPABILITY_NAMED_IAM \
  --parameter-overrides Phase=2 TableBucketName=otlp2pipeline NamespaceName=default
```

Phase 2 (Phase parameter = 2) creates:
- Kinesis Firehose delivery stream targeting the S3 Table

## Testing

Send a test record to Firehose:

```bash
./scripts/aws-send-test-record.sh otlp2pipeline-prod us-east-1
```

This script:
1. Creates a properly formatted OTLP log record
2. Base64 encodes and sends it to Firehose
3. Shows how to check delivery metrics

To verify delivery succeeded (wait ~60 seconds for Firehose buffering):

```bash
aws cloudwatch get-metric-statistics \
  --namespace AWS/Firehose \
  --metric-name DeliveryToIceberg.SuccessfulRowCount \
  --dimensions Name=DeliveryStreamName,Value=otlp2pipeline-prod \
  --start-time $(date -u -v-5M '+%Y-%m-%dT%H:%M:%SZ') \
  --end-time $(date -u '+%Y-%m-%dT%H:%M:%SZ') \
  --period 60 --statistics Sum --region us-east-1
```

## Troubleshooting

### "Insufficient Glue permissions to access database"

This error occurs when:
1. The S3 Tables integration isn't properly set up (run Phase 0 script)
2. The caller isn't a LakeFormation admin
3. The caller doesn't have permissions on the s3tablescatalog

Fix: Re-run `./scripts/aws-s3tables-setup.sh` and ensure you're using the same IAM identity that ran the setup.

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

**Fix:** Re-run `./scripts/aws-s3tables-setup.sh` - it will update the existing role with correct policies.

### "Catalog not found"

The `s3tablescatalog` federated catalog doesn't exist. Re-run the Phase 0 setup script.

## AWS Resources

- [S3 Tables User Guide](https://docs.aws.amazon.com/AmazonS3/latest/userguide/s3-tables.html)
- [Granting LakeFormation Permissions to S3 Tables](https://docs.aws.amazon.com/AmazonS3/latest/userguide/grant-permissions-tables.html)
- [S3 Tables + Lake Formation Prerequisites](https://docs.aws.amazon.com/lake-formation/latest/dg/s3tables-catalog-prerequisites.html)

## Known AWS Bugs

- [aws-cli #9618](https://github.com/aws/aws-cli/issues/9618): `grant-permissions --catalog-id` doesn't accept s3tablescatalog format
- https://docs.aws.amazon.com/firehose/latest/dev/apache-iceberg-destination.html
- [terraform-provider-aws #40724](https://github.com/hashicorp/terraform-provider-aws/issues/40724): LakeFormation permissions for S3Tables Catalog validation error
