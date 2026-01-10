#!/bin/bash
#
# Phase 2: Grant LakeFormation permissions to Firehose role
#
# Run this after Phase 1 (CloudFormation deploy with Phase=1) and before
# Phase 3 (CloudFormation deploy with Phase=2).
#
# Prerequisites:
# - Phase 0 setup script has been run
# - Phase 1 CloudFormation stack deployed successfully
# - Caller must be a LakeFormation admin
#
# Usage:
#   ./scripts/aws-grant-firehose-permissions.sh <stack-name> [region] [bucket-name] [namespace]
#   ./scripts/aws-grant-firehose-permissions.sh otlp2pipeline-prod us-east-1 otlp2pipeline default
#

set -e

STACK_NAME="${1:?Usage: $0 <stack-name> [region] [bucket-name] [namespace]}"
REGION="${2:-us-east-1}"
BUCKET_NAME="${3:-otlp2pipeline}"
NAMESPACE="${4:-default}"

echo "==> Granting LakeFormation permissions to Firehose role"
echo "    Stack: ${STACK_NAME}"
echo "    Region: ${REGION}"
echo "    Bucket: ${BUCKET_NAME}"
echo "    Namespace: ${NAMESPACE}"

# Get account ID
ACCOUNT_ID=$(aws sts get-caller-identity --query Account --output text)
echo "    Account ID: ${ACCOUNT_ID}"

# Get Firehose role ARN from CloudFormation outputs
echo ""
echo "==> Getting Firehose role ARN from CloudFormation stack..."
ROLE_ARN=$(aws cloudformation describe-stacks \
    --stack-name "${STACK_NAME}" \
    --query 'Stacks[0].Outputs[?OutputKey==`FirehoseRoleARN`].OutputValue' \
    --output text \
    --region "${REGION}")

if [ -z "${ROLE_ARN}" ] || [ "${ROLE_ARN}" = "None" ]; then
    echo "ERROR: Could not find FirehoseRoleARN in stack outputs."
    echo "       Make sure Phase 1 deployment completed successfully."
    exit 1
fi

echo "    Firehose Role: ${ROLE_ARN}"

# Grant DESCRIBE on database (namespace)
echo ""
echo "==> Granting DESCRIBE on database '${NAMESPACE}'..."
aws lakeformation grant-permissions \
    --region "${REGION}" \
    --principal "{\"DataLakePrincipalIdentifier\":\"${ROLE_ARN}\"}" \
    --resource "{\"Database\":{\"CatalogId\":\"${ACCOUNT_ID}:s3tablescatalog/${BUCKET_NAME}\",\"Name\":\"${NAMESPACE}\"}}" \
    --permissions DESCRIBE 2>/dev/null || echo "    (permission may already exist)"

echo "    Done"

# Grant ALL on table
echo ""
echo "==> Granting ALL on table 'logs'..."
aws lakeformation grant-permissions \
    --region "${REGION}" \
    --principal "{\"DataLakePrincipalIdentifier\":\"${ROLE_ARN}\"}" \
    --resource "{\"Table\":{\"CatalogId\":\"${ACCOUNT_ID}:s3tablescatalog/${BUCKET_NAME}\",\"DatabaseName\":\"${NAMESPACE}\",\"Name\":\"logs\"}}" \
    --permissions ALL 2>/dev/null || echo "    (permission may already exist)"

echo "    Done"

echo ""
echo "=========================================="
echo "LakeFormation permissions granted"
echo "=========================================="
echo ""
echo "You can now deploy Phase 2 (Firehose):"
echo ""
echo "  aws cloudformation deploy \\"
echo "    --template-file template.yaml \\"
echo "    --stack-name ${STACK_NAME} \\"
echo "    --region ${REGION} \\"
echo "    --capabilities CAPABILITY_NAMED_IAM \\"
echo "    --parameter-overrides Phase=2 TableBucketName=${BUCKET_NAME} NamespaceName=${NAMESPACE}"
echo ""
