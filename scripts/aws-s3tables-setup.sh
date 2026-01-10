#!/bin/bash
#
# Phase 0: Enable S3 Tables + Lake Formation Integration
#
# This script sets up the prerequisites for using S3 Tables with Lake Formation.
# Run this once per AWS account/region before deploying otlp2pipeline stacks.
#
# Prerequisites:
# - AWS CLI configured with credentials
# - Caller must have IAM and LakeFormation admin permissions
#
# Usage:
#   ./scripts/aws-s3tables-setup.sh [region]
#   ./scripts/aws-s3tables-setup.sh us-east-1
#

set -e

REGION="${1:-us-east-1}"
ROLE_NAME="S3TablesRoleForLakeFormation"

echo "==> Setting up S3 Tables + Lake Formation integration in ${REGION}"

# Get account ID
ACCOUNT_ID=$(aws sts get-caller-identity --query Account --output text)
CALLER_ARN=$(aws sts get-caller-identity --query Arn --output text)
echo "    Account ID: ${ACCOUNT_ID}"
echo "    Caller: ${CALLER_ARN}"

# Policy documents
# Trust policy must include sts:SetSourceIdentity and sts:SetContext for Lake Formation credential vending
# See: https://docs.aws.amazon.com/lake-formation/latest/dg/s3tables-catalog-prerequisites.html
TRUST_POLICY='{
    "Version": "2012-10-17",
    "Statement": [{
        "Effect": "Allow",
        "Principal": {"Service": "lakeformation.amazonaws.com"},
        "Action": ["sts:AssumeRole", "sts:SetSourceIdentity", "sts:SetContext"]
    }]
}'

S3_TABLES_POLICY='{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Sid": "LakeFormationPermissionsForS3ListTableBucket",
            "Effect": "Allow",
            "Action": ["s3tables:ListTableBuckets"],
            "Resource": ["*"]
        },
        {
            "Sid": "LakeFormationDataAccessPermissionsForS3TableBucket",
            "Effect": "Allow",
            "Action": [
                "s3tables:CreateTableBucket",
                "s3tables:GetTableBucket",
                "s3tables:CreateNamespace",
                "s3tables:GetNamespace",
                "s3tables:ListNamespaces",
                "s3tables:DeleteNamespace",
                "s3tables:DeleteTableBucket",
                "s3tables:CreateTable",
                "s3tables:DeleteTable",
                "s3tables:GetTable",
                "s3tables:ListTables",
                "s3tables:RenameTable",
                "s3tables:UpdateTableMetadataLocation",
                "s3tables:GetTableMetadataLocation",
                "s3tables:GetTableData",
                "s3tables:PutTableData"
            ],
            "Resource": ["*"]
        }
    ]
}'

# Step 1: Create or update the S3 Tables role for Lake Formation
echo ""
echo "==> Step 1: Creating IAM role for Lake Formation data access"

ROLE_CREATED=false
if aws iam get-role --role-name "${ROLE_NAME}" >/dev/null 2>&1; then
    echo "    Role ${ROLE_NAME} already exists, updating policies..."
    aws iam update-assume-role-policy \
        --role-name "${ROLE_NAME}" \
        --policy-document "${TRUST_POLICY}" >/dev/null
else
    echo "    Creating role ${ROLE_NAME}..."
    aws iam create-role \
        --role-name "${ROLE_NAME}" \
        --assume-role-policy-document "${TRUST_POLICY}" \
        --region "${REGION}" >/dev/null
    ROLE_CREATED=true
fi

# Always update inline policy (idempotent)
aws iam put-role-policy \
    --role-name "${ROLE_NAME}" \
    --policy-name "S3TablesDataAccess" \
    --policy-document "${S3_TABLES_POLICY}" >/dev/null

if [ "${ROLE_CREATED}" = true ]; then
    echo "    Waiting for IAM propagation..."
    sleep 10
fi

echo "    Done"

ROLE_ARN="arn:aws:iam::${ACCOUNT_ID}:role/${ROLE_NAME}"

# Step 2: Add caller as Lake Formation admin
echo ""
echo "==> Step 2: Adding caller as Lake Formation admin"

aws lakeformation put-data-lake-settings \
    --data-lake-settings "{\"DataLakeAdmins\":[{\"DataLakePrincipalIdentifier\":\"${CALLER_ARN}\"}]}" \
    --region "${REGION}" 2>/dev/null || true

echo "    Done"

# Step 3: Register S3 Tables resource with Lake Formation
echo ""
echo "==> Step 3: Registering S3 Tables resource with Lake Formation"

RESOURCE_ARN="arn:aws:s3tables:${REGION}:${ACCOUNT_ID}:bucket/*"

# Deregister first if exists
aws lakeformation deregister-resource \
    --resource-arn "${RESOURCE_ARN}" \
    --region "${REGION}" 2>/dev/null || true

# Register with federation
aws lakeformation register-resource \
    --resource-arn "${RESOURCE_ARN}" \
    --role-arn "${ROLE_ARN}" \
    --with-federation \
    --region "${REGION}" 2>/dev/null || true

echo "    Registered: ${RESOURCE_ARN}"

# Step 4: Create or update the s3tablescatalog federated catalog
echo ""
echo "==> Step 4: Creating/updating s3tablescatalog federated catalog"

# Delete existing catalog if present
aws glue delete-catalog \
    --catalog-id "s3tablescatalog" \
    --region "${REGION}" 2>/dev/null || true

# Create catalog with AllowFullTableExternalDataAccess
aws glue create-catalog \
    --name "s3tablescatalog" \
    --catalog-input "{
        \"FederatedCatalog\": {
            \"Identifier\": \"${RESOURCE_ARN}\",
            \"ConnectionName\": \"aws:s3tables\"
        },
        \"CreateDatabaseDefaultPermissions\": [],
        \"CreateTableDefaultPermissions\": [],
        \"CatalogProperties\": {
            \"CustomProperties\": {
                \"AllowFullTableExternalDataAccess\": \"true\"
            }
        }
    }" \
    --region "${REGION}" 2>/dev/null || true

echo "    Catalog created"

# Step 5: Grant catalog permissions to caller
echo ""
echo "==> Step 5: Granting catalog permissions"

aws lakeformation grant-permissions \
    --principal "{\"DataLakePrincipalIdentifier\":\"${CALLER_ARN}\"}" \
    --resource "{\"Catalog\":{\"Id\":\"${ACCOUNT_ID}:s3tablescatalog\"}}" \
    --permissions "ALL" "DESCRIBE" "CREATE_DATABASE" "ALTER" "DROP" \
    --permissions-with-grant-option "ALL" "DESCRIBE" "CREATE_DATABASE" "ALTER" "DROP" \
    --region "${REGION}" 2>/dev/null || echo "    (permissions may already exist)"

echo ""
echo "=========================================="
echo "S3 Tables + Lake Formation setup complete"
echo "=========================================="
echo ""
echo "You can now deploy otlp2pipeline stacks in ${REGION}."
echo ""
echo "Note: This setup creates:"
echo "  - IAM role: ${ROLE_NAME}"
echo "  - LakeFormation resource: ${RESOURCE_ARN}"
echo "  - Glue catalog: s3tablescatalog"
