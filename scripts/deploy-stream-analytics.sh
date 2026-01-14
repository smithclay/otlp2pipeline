#!/usr/bin/env bash
# Manage Azure Stream Analytics PoC infrastructure
# Supports: deploy, destroy
# This script is idempotent and can be safely rerun

set -euo pipefail

# Configuration
RESOURCE_GROUP="${RESOURCE_GROUP:-fabrictest01}"
LOCATION="${LOCATION:-westus}"
TEMPLATE="examples/azure-stream-analytics.bicep"
DEPLOYMENT_NAME="stream-analytics-deployment"
JOB_NAME="otlp-stream-processor"
EVENTHUB_NAMESPACE="otlp-poc-hub-test"
EVENTHUB_NAME="otlp-ingestion"
STORAGE_ACCOUNT="otlppocadls"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Parse command
COMMAND="${1:-deploy}"

show_help() {
    cat << EOF
Usage: $0 [COMMAND]

Commands:
  deploy    Deploy Stream Analytics infrastructure (default)
  destroy   Destroy all PoC resources

Environment Variables:
  RESOURCE_GROUP     Azure resource group (default: fabrictest01)
  LOCATION          Azure region (default: westus)

Examples:
  $0 deploy          # Deploy infrastructure
  $0 destroy         # Clean up all resources
  $0 destroy --force # Skip confirmation prompt

EOF
    exit 0
}

deploy_infrastructure() {
    echo -e "${BLUE}🚀 Azure Stream Analytics Deployment${NC}"
    echo "======================================"
    echo ""

    # Check if running from project root
    if [ ! -f "$TEMPLATE" ]; then
        echo -e "${YELLOW}⚠️  Template not found: $TEMPLATE${NC}"
        echo "   Please run this script from the project root directory"
        exit 1
    fi

    echo -e "${BLUE}📋 Configuration:${NC}"
    echo "   Resource Group: $RESOURCE_GROUP"
    echo "   Location: $LOCATION"
    echo "   Template: $TEMPLATE"
    echo "   Job Name: $JOB_NAME"
    echo ""

    # Retrieve Event Hub access key
    echo -e "${BLUE}🔑 Retrieving Event Hub access key...${NC}"
    EVENTHUB_KEY=$(az eventhubs namespace authorization-rule keys list \
      --resource-group "$RESOURCE_GROUP" \
      --namespace-name "$EVENTHUB_NAMESPACE" \
      --name RootManageSharedAccessKey \
      --query primaryKey -o tsv 2>/dev/null || echo "")

    if [ -z "$EVENTHUB_KEY" ]; then
        echo -e "${YELLOW}⚠️  Failed to retrieve Event Hub key${NC}"
        exit 1
    fi
    echo -e "${GREEN}✅ Event Hub key retrieved${NC}"

    # Retrieve Storage Account access key
    echo -e "${BLUE}🔑 Retrieving Storage Account key...${NC}"
    STORAGE_KEY=$(az storage account keys list \
      --resource-group "$RESOURCE_GROUP" \
      --account-name "$STORAGE_ACCOUNT" \
      --query '[0].value' -o tsv 2>/dev/null || echo "")

    if [ -z "$STORAGE_KEY" ]; then
        echo -e "${YELLOW}⚠️  Failed to retrieve Storage Account key${NC}"
        exit 1
    fi
    echo -e "${GREEN}✅ Storage key retrieved${NC}"
    echo ""

    # Deploy Bicep template
    echo -e "${BLUE}📦 Deploying Bicep template...${NC}"
    echo "   This may take 2-3 minutes..."
    echo ""

    az deployment group create \
      --resource-group "$RESOURCE_GROUP" \
      --name "$DEPLOYMENT_NAME" \
      --template-file "$TEMPLATE" \
      --parameters \
        location="$LOCATION" \
        streamAnalyticsJobName="$JOB_NAME" \
        eventHubNamespace="$EVENTHUB_NAMESPACE" \
        eventHubName="$EVENTHUB_NAME" \
        eventHubSharedAccessPolicyKey="$EVENTHUB_KEY" \
        storageAccountName="$STORAGE_ACCOUNT" \
        storageAccountKey="$STORAGE_KEY" \
      --output table

    echo ""
    echo -e "${GREEN}✅ Bicep template deployed successfully${NC}"
    echo ""

    # Check current job state
    echo -e "${BLUE}📊 Checking Stream Analytics job state...${NC}"
    JOB_STATE=$(az stream-analytics job show \
      --resource-group "$RESOURCE_GROUP" \
      --name "$JOB_NAME" \
      --query jobState -o tsv 2>/dev/null || echo "Unknown")

    echo "   Current state: $JOB_STATE"
    echo ""

    # Start the job if not already running
    if [ "$JOB_STATE" != "Running" ]; then
        echo -e "${BLUE}▶️  Starting Stream Analytics job...${NC}"
        az stream-analytics job start \
          --resource-group "$RESOURCE_GROUP" \
          --name "$JOB_NAME" \
          --output-start-mode JobStartTime \
          --output table

        echo -e "${GREEN}✅ Stream Analytics job started${NC}"
    else
        echo -e "${GREEN}✅ Stream Analytics job is already running${NC}"
    fi
    echo ""

    # Validate deployment
    echo -e "${BLUE}🔍 Validating deployment...${NC}"
    echo ""

    # Check inputs
    echo "Inputs:"
    az stream-analytics input list \
      --job-name "$JOB_NAME" \
      --resource-group "$RESOURCE_GROUP" \
      --query '[].name' -o table

    echo ""

    # Check outputs
    echo "Outputs:"
    az stream-analytics output list \
      --job-name "$JOB_NAME" \
      --resource-group "$RESOURCE_GROUP" \
      --query '[].name' -o table

    echo ""

    # Final job status
    echo "Job Status:"
    az stream-analytics job show \
      --resource-group "$RESOURCE_GROUP" \
      --name "$JOB_NAME" \
      --query '{Name:name,State:jobState,StreamingUnits:transformation.streamingUnits}' \
      --output table

    echo ""
    echo -e "${GREEN}🎉 Deployment Complete!${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Run the PoC: cargo run --example azure_eventhub_poc --features azure"
    echo "  2. Wait 5 minutes for batching window"
    echo "  3. Check for Parquet files:"
    echo "     az storage blob list --container-name logs --account-name $STORAGE_ACCOUNT --auth-mode key"
    echo ""
}

destroy_infrastructure() {
    echo -e "${RED}🗑️  Azure Stream Analytics PoC Cleanup${NC}"
    echo "========================================="
    echo ""

    echo -e "${BLUE}📋 Resources to be deleted:${NC}"
    echo "   Resource Group: $RESOURCE_GROUP"
    echo "   - Stream Analytics Job: $JOB_NAME"
    echo "   - Event Hub Namespace: $EVENTHUB_NAMESPACE"
    echo "   - Event Hub: $EVENTHUB_NAME"
    echo "   - Storage Account: $STORAGE_ACCOUNT"
    echo "   - Storage Containers: logs, traces, metrics, otlp-capture"
    echo ""

    # Check for --force flag
    FORCE=false
    if [[ "${2:-}" == "--force" ]]; then
        FORCE=true
    fi

    # Confirmation prompt (unless --force)
    if [ "$FORCE" = false ]; then
        echo -e "${YELLOW}⚠️  WARNING: This will permanently delete all PoC resources!${NC}"
        echo -e "${YELLOW}   This action cannot be undone.${NC}"
        echo ""
        read -p "Are you sure you want to continue? (type 'yes' to confirm): " -r
        echo ""
        if [[ ! $REPLY =~ ^yes$ ]]; then
            echo "Aborted."
            exit 0
        fi
    fi

    echo -e "${BLUE}🗑️  Starting cleanup...${NC}"
    echo ""

    # 1. Stop and delete Stream Analytics job
    echo -e "${BLUE}Stopping Stream Analytics job...${NC}"
    JOB_STATE=$(az stream-analytics job show \
      --resource-group "$RESOURCE_GROUP" \
      --name "$JOB_NAME" \
      --query jobState -o tsv 2>/dev/null || echo "NotFound")

    if [ "$JOB_STATE" = "Running" ]; then
        az stream-analytics job stop \
          --resource-group "$RESOURCE_GROUP" \
          --name "$JOB_NAME" \
          --output none 2>/dev/null || true
        echo -e "${GREEN}✅ Stream Analytics job stopped${NC}"
    elif [ "$JOB_STATE" != "NotFound" ]; then
        echo -e "${YELLOW}   Job not running (state: $JOB_STATE)${NC}"
    fi

    if [ "$JOB_STATE" != "NotFound" ]; then
        echo -e "${BLUE}Deleting Stream Analytics job...${NC}"
        az stream-analytics job delete \
          --resource-group "$RESOURCE_GROUP" \
          --name "$JOB_NAME" \
          --yes \
          --output none 2>/dev/null || true
        echo -e "${GREEN}✅ Stream Analytics job deleted${NC}"
    else
        echo -e "${YELLOW}   Stream Analytics job not found (already deleted)${NC}"
    fi
    echo ""

    # 2. Delete Event Hub
    echo -e "${BLUE}Deleting Event Hub...${NC}"
    az eventhubs eventhub delete \
      --resource-group "$RESOURCE_GROUP" \
      --namespace-name "$EVENTHUB_NAMESPACE" \
      --name "$EVENTHUB_NAME" \
      --output none 2>/dev/null || echo -e "${YELLOW}   Event Hub not found (already deleted)${NC}"
    echo -e "${GREEN}✅ Event Hub deleted${NC}"
    echo ""

    # 3. Delete Event Hub namespace
    echo -e "${BLUE}Deleting Event Hub namespace...${NC}"
    az eventhubs namespace delete \
      --resource-group "$RESOURCE_GROUP" \
      --name "$EVENTHUB_NAMESPACE" \
      --output none 2>/dev/null || echo -e "${YELLOW}   Event Hub namespace not found (already deleted)${NC}"
    echo -e "${GREEN}✅ Event Hub namespace deleted${NC}"
    echo ""

    # 4. Delete storage containers
    echo -e "${BLUE}Deleting storage containers...${NC}"
    for container in logs traces metrics otlp-capture; do
        az storage container delete \
          --name "$container" \
          --account-name "$STORAGE_ACCOUNT" \
          --auth-mode key \
          --output none 2>/dev/null && echo "   ✓ Deleted: $container" || echo "   ⊘ Not found: $container"
    done
    echo -e "${GREEN}✅ Storage containers deleted${NC}"
    echo ""

    # 5. Delete storage account
    echo -e "${BLUE}Deleting storage account...${NC}"
    az storage account delete \
      --resource-group "$RESOURCE_GROUP" \
      --name "$STORAGE_ACCOUNT" \
      --yes \
      --output none 2>/dev/null || echo -e "${YELLOW}   Storage account not found (already deleted)${NC}"
    echo -e "${GREEN}✅ Storage account deleted${NC}"
    echo ""

    echo -e "${GREEN}🎉 Cleanup Complete!${NC}"
    echo ""
    echo "All PoC resources have been deleted from resource group: $RESOURCE_GROUP"
    echo ""
    echo "Note: Resource group '$RESOURCE_GROUP' was preserved."
    echo "To delete it: az group delete --name $RESOURCE_GROUP --yes"
    echo ""
}

# Main
case "$COMMAND" in
    deploy)
        deploy_infrastructure
        ;;
    destroy)
        destroy_infrastructure "$@"
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        echo -e "${RED}Error: Unknown command '$COMMAND'${NC}"
        echo ""
        show_help
        ;;
esac
