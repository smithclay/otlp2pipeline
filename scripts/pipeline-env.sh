#!/usr/bin/env bash
set -eo pipefail

# Pipeline environment management using Cloudflare REST API
# Usage:
#   ./scripts/pipeline-env.sh create <env-name>
#   ./scripts/pipeline-env.sh delete <env-name>
#   ./scripts/pipeline-env.sh status <env-name>
#   ./scripts/pipeline-env.sh dry-run <env-name>
#   ./scripts/pipeline-env.sh query <env-name>
#   ./scripts/pipeline-env.sh configure <env-name>
#
# Environment variables (all optional - will auto-detect from wrangler):
#   CF_API_TOKEN  - Cloudflare API token (falls back to wrangler OAuth token)
#   CF_ACCOUNT_ID - Cloudflare account ID (auto-detects from API)

ACTION="${1:-}"
ENV_NAME="${2:-}"
SERVICE_TOKEN=""

# Parse --token flag for create/configure commands
# Supports: <action> <env> --token <token> or <action> --token <token> <env>
if [[ "${3:-}" == "--token" ]]; then
    SERVICE_TOKEN="${4:-}"
elif [[ "$ENV_NAME" == "--token" ]]; then
    SERVICE_TOKEN="${3:-}"
    ENV_NAME="${4:-}"
fi

if [[ -z "$ACTION" || -z "$ENV_NAME" ]]; then
    echo "Usage: $0 <create|delete|status|dry-run|query|configure> <env-name> [options]"
    echo ""
    echo "Examples:"
    echo "  $0 create test01 --token <R2_TOKEN>    # Create environment with service credential"
    echo "  $0 delete test01                       # Tear down environment"
    echo "  $0 status test01                       # Check environment status"
    echo "  $0 dry-run test01                      # Show what would be created"
    echo "  $0 query test01                        # Start DuckDB session for querying"
    echo "  $0 configure test01 --token <R2_TOKEN> # Configure catalog maintenance"
    echo ""
    echo "The --token flag is required for create/configure commands."
    echo "Create an R2 API token at: https://dash.cloudflare.com/?to=/:account/r2/api-tokens"
    echo ""
    echo "Auth: Uses wrangler OAuth token automatically, or set CF_API_TOKEN"
    echo "      For query command, also set R2_API_TOKEN or enter interactively"
    exit 1
fi

# Check for API token - try env var first, then wrangler's OAuth token
if [[ -z "${CF_API_TOKEN:-}" ]]; then
    # Try to get wrangler's OAuth token
    WRANGLER_CONFIG="${HOME}/Library/Preferences/.wrangler/config/default.toml"
    if [[ ! -f "$WRANGLER_CONFIG" ]]; then
        # Try Linux/other location
        WRANGLER_CONFIG="${HOME}/.wrangler/config/default.toml"
    fi

    if [[ -f "$WRANGLER_CONFIG" ]]; then
        echo "==> Using wrangler OAuth token..."
        # Strip ANSI codes and extract token
        CF_API_TOKEN=$(sed 's/\x1b\[[0-9;]*m//g' "$WRANGLER_CONFIG" | grep '^oauth_token' | cut -d'"' -f2)
        EXPIRATION=$(sed 's/\x1b\[[0-9;]*m//g' "$WRANGLER_CONFIG" | grep '^expiration_time' | cut -d'"' -f2)

        if [[ -z "$CF_API_TOKEN" ]]; then
            echo "Error: Could not extract OAuth token from wrangler config"
            echo "Run 'npx wrangler login' or set CF_API_TOKEN"
            exit 1
        fi

        # Check if token is expired
        if [[ -n "$EXPIRATION" ]]; then
            EXPIRATION_TS=$(date -j -f "%Y-%m-%dT%H:%M:%S" "${EXPIRATION%%.*}" "+%s" 2>/dev/null || date -d "${EXPIRATION}" "+%s" 2>/dev/null || echo "0")
            NOW_TS=$(date "+%s")
            if [[ "$EXPIRATION_TS" -lt "$NOW_TS" ]]; then
                echo "Warning: Wrangler OAuth token may be expired (${EXPIRATION})"
                echo "Run 'npx wrangler login' to refresh, or set CF_API_TOKEN"
            fi
        fi
    else
        echo "Error: CF_API_TOKEN not set and no wrangler config found"
        echo ""
        echo "Either:"
        echo "  1. Run 'npx wrangler login' to authenticate"
        echo "  2. Set CF_API_TOKEN environment variable"
        echo "     Create one at: https://dash.cloudflare.com/profile/api-tokens"
        exit 1
    fi
fi

# Validate token by fetching accounts (works for both API tokens and OAuth tokens)
echo "==> Validating token and detecting account..."
ACCOUNTS_RESPONSE=$(curl -s -H "Authorization: Bearer ${CF_API_TOKEN}" \
    "https://api.cloudflare.com/client/v4/accounts")

if ! echo "$ACCOUNTS_RESPONSE" | jq -e '.success' >/dev/null 2>&1; then
    echo "Error: Invalid or expired token"
    echo "$ACCOUNTS_RESPONSE" | jq -r '.errors[0].message // "Unknown error"' 2>/dev/null
    echo ""
    echo "Try: npx wrangler login"
    exit 1
fi
echo "    Token valid"

# Auto-detect account ID if not provided
if [[ -z "${CF_ACCOUNT_ID:-}" ]]; then
    CF_ACCOUNT_ID=$(echo "$ACCOUNTS_RESPONSE" | jq -r '.result[0].id')
    if [[ -z "$CF_ACCOUNT_ID" || "$CF_ACCOUNT_ID" == "null" ]]; then
        echo "Error: Could not detect account ID. Set CF_ACCOUNT_ID manually."
        exit 1
    fi
fi
echo "    Account ID: ${CF_ACCOUNT_ID}"

# API base URL
API_BASE="https://api.cloudflare.com/client/v4/accounts/${CF_ACCOUNT_ID}/pipelines/v1"

# Naming conventions
# R2 buckets: lowercase alphanumeric + dashes only
# Streams/sinks/pipelines: alphanumeric + underscores only
BUCKET_SAFE="${ENV_NAME//_/-}"
STREAM_SAFE="${ENV_NAME//-/_}"
BUCKET_NAME="otlpflare-${BUCKET_SAFE}"

# Signal types and their schema files
SIGNAL_NAMES=("logs" "traces" "gauge" "sum")
SIGNAL_SCHEMAS=("schemas/logs.schema.json" "schemas/spans.schema.json" "schemas/gauge.schema.json" "schemas/sum.schema.json")

# Resource naming functions
stream_name() { echo "otlpflare_${STREAM_SAFE}_${1}"; }
sink_name() { echo "otlpflare_${STREAM_SAFE}_${1}_sink"; }
pipeline_name() { echo "otlpflare_${STREAM_SAFE}_${1}"; }

# API helper functions
api_get() {
    curl -s -H "Authorization: Bearer ${CF_API_TOKEN}" \
        -H "Content-Type: application/json" \
        "${API_BASE}${1}"
}

api_post() {
    curl -s -X POST -H "Authorization: Bearer ${CF_API_TOKEN}" \
        -H "Content-Type: application/json" \
        -d "$2" \
        "${API_BASE}${1}"
}

api_delete() {
    curl -s -X DELETE -H "Authorization: Bearer ${CF_API_TOKEN}" \
        -H "Content-Type: application/json" \
        "${API_BASE}${1}"
}

# Get resource ID by name
get_stream_id() {
    api_get "/streams" | jq -r ".result[] | select(.name == \"$1\") | .id"
}

get_sink_id() {
    api_get "/sinks" | jq -r ".result[] | select(.name == \"$1\") | .id"
}

get_pipeline_id() {
    api_get "/pipelines" | jq -r ".result[] | select(.name == \"$1\") | .id"
}

# R2 bucket management (still uses wrangler - no direct API for bucket creation)
# Note: Maintenance settings (compaction, snapshot expiration) configured via configure_catalog()
create_bucket() {
    npx wrangler r2 bucket create "$1" 2>/dev/null || true
    npx wrangler r2 bucket catalog enable "$1" 2>/dev/null || true
}

delete_bucket() {
    npx wrangler r2 bucket delete "$1" 2>/dev/null || true
}

dry_run() {
    echo "==> Dry run for environment: ${ENV_NAME}"
    echo ""
    echo "Would create:"
    echo "  R2 Bucket: ${BUCKET_NAME}"
    echo ""
    echo "  Streams:"
    for i in "${!SIGNAL_NAMES[@]}"; do
        echo "    - $(stream_name "${SIGNAL_NAMES[$i]}") (schema: ${SIGNAL_SCHEMAS[$i]})"
    done
    echo ""
    echo "  Sinks:"
    for signal in "${SIGNAL_NAMES[@]}"; do
        echo "    - $(sink_name "$signal") -> table: ${signal}"
    done
    echo ""
    echo "  Pipelines:"
    for signal in "${SIGNAL_NAMES[@]}"; do
        echo "    - $(pipeline_name "$signal")"
    done
    echo ""
    echo "Checking current state..."
    echo ""

    # Check streams
    local streams_json
    streams_json=$(api_get "/streams")
    for signal in "${SIGNAL_NAMES[@]}"; do
        local name id
        name=$(stream_name "$signal")
        id=$(echo "$streams_json" | jq -r ".result[] | select(.name == \"$name\") | .id")
        if [[ -n "$id" ]]; then
            echo "  Stream $name: EXISTS ($id)"
        else
            echo "  Stream $name: not found"
        fi
    done

    # Check sinks
    local sinks_json
    sinks_json=$(api_get "/sinks")
    for signal in "${SIGNAL_NAMES[@]}"; do
        local name id
        name=$(sink_name "$signal")
        id=$(echo "$sinks_json" | jq -r ".result[] | select(.name == \"$name\") | .id")
        if [[ -n "$id" ]]; then
            echo "  Sink $name: EXISTS ($id)"
        else
            echo "  Sink $name: not found"
        fi
    done

    # Check pipelines
    local pipelines_json
    pipelines_json=$(api_get "/pipelines")
    for signal in "${SIGNAL_NAMES[@]}"; do
        local name id
        name=$(pipeline_name "$signal")
        id=$(echo "$pipelines_json" | jq -r ".result[] | select(.name == \"$name\") | .id")
        if [[ -n "$id" ]]; then
            echo "  Pipeline $name: EXISTS ($id)"
        else
            echo "  Pipeline $name: not found"
        fi
    done
}

create_env() {
    echo "==> Creating pipeline environment: ${ENV_NAME}"
    echo "    Bucket: ${BUCKET_NAME}"
    echo "    Signals: ${SIGNAL_NAMES[*]}"
    echo ""

    # Step 1: Create R2 bucket
    echo "==> Creating R2 bucket: ${BUCKET_NAME}"
    create_bucket "${BUCKET_NAME}"
    echo "    Done"

    # Step 1b: Configure catalog maintenance (compaction + snapshot expiration)
    configure_catalog

    # Step 2: Create streams
    echo ""
    echo "==> Creating streams..."
    for i in "${!SIGNAL_NAMES[@]}"; do
        local signal="${SIGNAL_NAMES[$i]}"
        local schema="${SIGNAL_SCHEMAS[$i]}"
        local name
        name=$(stream_name "$signal")

        echo "    Creating: ${name}"

        if [[ ! -f "$schema" ]]; then
            echo "      WARNING: Schema file not found: ${schema}"
            continue
        fi

        local schema_json
        schema_json=$(jq -c '.fields' "$schema")

        local response
        response=$(api_post "/streams" "{
            \"name\": \"${name}\",
            \"format\": {\"type\": \"json\"},
            \"schema\": {\"fields\": ${schema_json}},
            \"http\": {\"enabled\": true, \"authentication\": true},
            \"worker_binding\": {\"enabled\": true}
        }")

        if echo "$response" | jq -e '.success' >/dev/null 2>&1; then
            echo "      Created"
        else
            local error
            error=$(echo "$response" | jq -r '.errors[0].message // "Unknown error"')
            echo "      Failed: ${error}"
        fi
    done

    # Step 3: Collect stream endpoints
    echo ""
    echo "==> Getting stream endpoints..."
    local streams_json endpoints_file
    streams_json=$(api_get "/streams")
    endpoints_file=$(mktemp)

    for signal in "${SIGNAL_NAMES[@]}"; do
        local name endpoint
        name=$(stream_name "$signal")
        endpoint=$(echo "$streams_json" | jq -r ".result[] | select(.name == \"$name\") | .endpoint")
        if [[ -n "$endpoint" && "$endpoint" != "null" ]]; then
            echo "${signal}=${endpoint}" >> "$endpoints_file"
            echo "    ${signal}: ${endpoint}"
        else
            echo "    ${signal}: NOT FOUND"
        fi
    done

    # Step 4: Get R2 API token from user
    echo ""
    echo "=========================================="
    echo "R2 API TOKEN REQUIRED"
    echo "=========================================="
    echo ""
    echo "Open this link to create an R2 API token:"
    echo "  https://dash.cloudflare.com/?to=/:account/r2/api-tokens"
    echo ""
    echo "Create a token with:"
    echo "  - Permissions: Admin Read & Write"
    echo "  - Specify bucket: ${BUCKET_NAME}"
    echo ""
    read -r -p "Paste your R2 API token here: " R2_TOKEN

    if [[ -z "$R2_TOKEN" ]]; then
        echo "Error: No token provided"
        rm -f "$endpoints_file"
        exit 1
    fi

    # Step 5: Create sinks
    echo ""
    echo "==> Creating sinks..."
    for signal in "${SIGNAL_NAMES[@]}"; do
        local name
        name=$(sink_name "$signal")
        echo "    Creating: ${name}"

        local response
        response=$(api_post "/sinks" "{
            \"name\": \"${name}\",
            \"type\": \"r2_data_catalog\",
            \"format\": {\"type\": \"parquet\", \"compression\": \"zstd\"},
            \"config\": {
                \"bucket\": \"${BUCKET_NAME}\",
                \"namespace\": \"default\",
                \"table_name\": \"${signal}\",
                \"token\": \"${R2_TOKEN}\"
            }
        }")

        if echo "$response" | jq -e '.success' >/dev/null 2>&1; then
            echo "      Created"
        else
            local error
            error=$(echo "$response" | jq -r '.errors[0].message // "Unknown error"')
            echo "      Failed: ${error}"
        fi
    done

    # Step 6: Create pipelines
    echo ""
    echo "==> Creating pipelines..."
    for signal in "${SIGNAL_NAMES[@]}"; do
        local name stream sink
        name=$(pipeline_name "$signal")
        stream=$(stream_name "$signal")
        sink=$(sink_name "$signal")
        echo "    Creating: ${name}"

        local response
        response=$(api_post "/pipelines" "{
            \"name\": \"${name}\",
            \"sql\": \"INSERT INTO ${sink} SELECT * FROM ${stream}\"
        }")

        if echo "$response" | jq -e '.success' >/dev/null 2>&1; then
            echo "      Created"
        else
            local error
            error=$(echo "$response" | jq -r '.errors[0].message // "Unknown error"')
            echo "      Failed: ${error}"
        fi
    done

    # Step 7: Generate wrangler.toml
    echo ""
    echo "==> Generating wrangler.toml..."

    local logs_endpoint traces_endpoint gauge_endpoint sum_endpoint
    logs_endpoint=$(grep "^logs=" "$endpoints_file" 2>/dev/null | cut -d= -f2 || echo "")
    traces_endpoint=$(grep "^traces=" "$endpoints_file" 2>/dev/null | cut -d= -f2 || echo "")
    gauge_endpoint=$(grep "^gauge=" "$endpoints_file" 2>/dev/null | cut -d= -f2 || echo "")
    sum_endpoint=$(grep "^sum=" "$endpoints_file" 2>/dev/null | cut -d= -f2 || echo "")
    rm -f "$endpoints_file"

    cat > wrangler.toml <<EOF
name = "otlpflare-${ENV_NAME}"
main = "build/worker/shim.mjs"
compatibility_date = "2024-01-01"

[build]
command = "cargo install -q worker-build && worker-build --release"

[vars]
PIPELINE_LOGS = "${logs_endpoint}"
PIPELINE_TRACES = "${traces_endpoint}"
PIPELINE_GAUGE = "${gauge_endpoint}"
PIPELINE_SUM = "${sum_endpoint}"
AGGREGATOR_ENABLED = "true"
AGGREGATOR_RETENTION_MINUTES = "60"

[observability]
enabled = true

[observability.logs]
invocation_logs = true
head_sampling_rate = 0.1

[observability.traces]
enabled = false

[[durable_objects.bindings]]
name = "AGGREGATOR"
class_name = "AggregatorDO"

[[migrations]]
tag = "v1"
new_sqlite_classes = ["AggregatorDO"]
EOF

    echo "    Created: wrangler.toml"

    # Step 8: Summary
    echo ""
    echo "=========================================="
    echo "ENVIRONMENT CREATED"
    echo "=========================================="
    echo ""
    echo "Generated wrangler.toml with:"
    echo "  name = \"otlpflare-${ENV_NAME}\""
    echo "  PIPELINE_LOGS = \"${logs_endpoint}\""
    echo "  PIPELINE_TRACES = \"${traces_endpoint}\""
    echo "  PIPELINE_GAUGE = \"${gauge_endpoint}\""
    echo "  PIPELINE_SUM = \"${sum_endpoint}\""
    echo ""
    echo "Next steps:"
    echo "  1. Set pipeline auth token:"
    echo "     npx wrangler secret put PIPELINE_AUTH_TOKEN"
    echo ""
    echo "  2. Deploy:"
    echo "     npx wrangler deploy"
}

delete_env() {
    echo "==> Deleting pipeline environment: ${ENV_NAME}"
    echo "    Bucket: ${BUCKET_NAME}"
    echo "    Signals: ${SIGNAL_NAMES[*]}"
    echo ""

    # Delete pipelines first (dependency order)
    echo "==> Deleting pipelines..."
    for signal in "${SIGNAL_NAMES[@]}"; do
        local name id
        name=$(pipeline_name "$signal")
        id=$(get_pipeline_id "$name")
        if [[ -n "$id" ]]; then
            echo "    Deleting: ${name} (${id})"
            local response
            response=$(api_delete "/pipelines/${id}")
            if echo "$response" | jq -e '.success' >/dev/null 2>&1; then
                echo "      Deleted"
            else
                echo "      Failed: $(echo "$response" | jq -r '.errors[0].message // "Unknown"')"
            fi
        else
            echo "    ${name}: not found"
        fi
    done

    # Delete sinks
    echo ""
    echo "==> Deleting sinks..."
    for signal in "${SIGNAL_NAMES[@]}"; do
        local name id
        name=$(sink_name "$signal")
        id=$(get_sink_id "$name")
        if [[ -n "$id" ]]; then
            echo "    Deleting: ${name} (${id})"
            local response
            response=$(api_delete "/sinks/${id}")
            if echo "$response" | jq -e '.success' >/dev/null 2>&1; then
                echo "      Deleted"
            else
                echo "      Failed: $(echo "$response" | jq -r '.errors[0].message // "Unknown"')"
            fi
        else
            echo "    ${name}: not found"
        fi
    done

    # Delete streams
    echo ""
    echo "==> Deleting streams..."
    for signal in "${SIGNAL_NAMES[@]}"; do
        local name id
        name=$(stream_name "$signal")
        id=$(get_stream_id "$name")
        if [[ -n "$id" ]]; then
            echo "    Deleting: ${name} (${id})"
            local response
            response=$(api_delete "/streams/${id}")
            if echo "$response" | jq -e '.success' >/dev/null 2>&1; then
                echo "      Deleted"
            else
                echo "      Failed: $(echo "$response" | jq -r '.errors[0].message // "Unknown"')"
            fi
        else
            echo "    ${name}: not found"
        fi
    done

    # Delete bucket
    echo ""
    echo "==> Deleting R2 bucket: ${BUCKET_NAME}"
    delete_bucket "${BUCKET_NAME}"

    echo ""
    echo "==> Done"
}

status_env() {
    echo "==> Pipeline environment status: ${ENV_NAME}"
    echo ""

    # Streams
    echo "==> Streams:"
    local streams_json
    streams_json=$(api_get "/streams")
    for signal in "${SIGNAL_NAMES[@]}"; do
        local name endpoint
        name=$(stream_name "$signal")
        endpoint=$(echo "$streams_json" | jq -r ".result[] | select(.name == \"$name\") | .endpoint")
        if [[ -n "$endpoint" && "$endpoint" != "null" ]]; then
            echo "    ${signal}: ${endpoint}"
        else
            echo "    ${signal}: NOT FOUND"
        fi
    done

    # Sinks
    echo ""
    echo "==> Sinks:"
    local sinks_json
    sinks_json=$(api_get "/sinks")
    for signal in "${SIGNAL_NAMES[@]}"; do
        local name id
        name=$(sink_name "$signal")
        id=$(echo "$sinks_json" | jq -r ".result[] | select(.name == \"$name\") | .id")
        if [[ -n "$id" && "$id" != "null" ]]; then
            echo "    ${signal}: ${name} (${id})"
        else
            echo "    ${signal}: NOT FOUND"
        fi
    done

    # Pipelines
    echo ""
    echo "==> Pipelines:"
    local pipelines_json
    pipelines_json=$(api_get "/pipelines")
    for signal in "${SIGNAL_NAMES[@]}"; do
        local name status
        name=$(pipeline_name "$signal")
        status=$(echo "$pipelines_json" | jq -r ".result[] | select(.name == \"$name\") | .status")
        if [[ -n "$status" && "$status" != "null" ]]; then
            echo "    ${signal}: ${name} (${status})"
        else
            echo "    ${signal}: NOT FOUND"
        fi
    done
}

query_env() {
    echo "==> Starting DuckDB session for environment: ${ENV_NAME}"
    echo "    Bucket: ${BUCKET_NAME}"
    echo ""

    # Check for duckdb
    if ! command -v duckdb &>/dev/null; then
        echo "Error: duckdb not found"
        echo ""
        echo "Install DuckDB (v1.4.0+ required for Iceberg REST Catalog):"
        echo "  brew install duckdb"
        echo "  # or download from https://duckdb.org/docs/installation/"
        exit 1
    fi

    # Check duckdb version
    local duckdb_version
    duckdb_version=$(duckdb -version 2>/dev/null | head -1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || echo "0.0.0")
    local major minor
    major=$(echo "$duckdb_version" | cut -d. -f1)
    minor=$(echo "$duckdb_version" | cut -d. -f2)
    if [[ "$major" -lt 1 ]] || [[ "$major" -eq 1 && "$minor" -lt 4 ]]; then
        echo "Warning: DuckDB version ${duckdb_version} detected"
        echo "         Version 1.4.0+ is required for Iceberg REST Catalog"
        echo ""
    fi

    # Get R2 API token
    local r2_token="${R2_API_TOKEN:-}"
    if [[ -z "$r2_token" ]]; then
        echo "R2 API token required for Data Catalog access."
        echo ""
        echo "Create one at: https://dash.cloudflare.com/?to=/:account/r2/api-tokens"
        echo "  - Permissions: Admin Read & Write"
        echo "  - Specify bucket: ${BUCKET_NAME}"
        echo ""
        echo "Tip: Set R2_API_TOKEN env var to skip this prompt"
        echo ""
        read -r -p "R2 API Token: " r2_token

        if [[ -z "$r2_token" ]]; then
            echo "Error: No token provided"
            exit 1
        fi
    else
        echo "    Using R2_API_TOKEN from environment"
    fi

    # Build catalog connection details
    local warehouse="${CF_ACCOUNT_ID}_${BUCKET_NAME}"
    local catalog_uri="https://catalog.cloudflarestorage.com/${CF_ACCOUNT_ID}/${BUCKET_NAME}"

    echo ""
    echo "    Warehouse: ${warehouse}"
    echo "    Catalog URI: ${catalog_uri}"
    echo ""

    # Create init SQL file (extension not required for DuckDB init)
    local init_file
    init_file="$(mktemp "${TMPDIR:-/tmp}/duckdb-init.XXXXXXXXXX")"

    cat > "$init_file" <<EOF
-- DuckDB init for otlpflare environment: ${ENV_NAME}
-- Auto-generated by pipeline-env.sh

-- Install and load required extensions
INSTALL iceberg;
LOAD iceberg;
INSTALL httpfs;
LOAD httpfs;

-- Create secret for R2 Data Catalog
CREATE SECRET r2_catalog_secret (
    TYPE ICEBERG,
    TOKEN '${r2_token}'
);

-- Attach R2 Data Catalog
ATTACH '${warehouse}' AS r2 (
    TYPE ICEBERG,
    ENDPOINT '${catalog_uri}'
);

-- Set default schema
USE r2.default;

-- Show available tables
.print ''
.print '==> Connected to R2 Data Catalog'
.print '    Catalog: r2'
.print '    Schema: default'
.print ''
.print 'Available tables:'
SHOW TABLES;

.print ''
.print 'Example queries:'
.print '  SELECT count(*) FROM logs;'
.print '  SELECT * FROM traces LIMIT 10;'
.print '  DESCRIBE logs;'
.print ''
EOF

    echo "==> Launching DuckDB..."
    echo ""

    # Launch duckdb with init file
    duckdb -init "$init_file"

    # Cleanup
    rm -f "$init_file"
}

# Configure R2 Data Catalog maintenance settings via REST API
configure_catalog() {
    echo "==> Configuring catalog maintenance for: ${BUCKET_NAME}"
    echo ""

    # Check if bucket exists (wrangler list output format: "  name  creation_date")
    if ! npx wrangler r2 bucket list 2>/dev/null | grep -q "${BUCKET_NAME}"; then
        echo "Error: Bucket ${BUCKET_NAME} does not exist"
        echo "Run '$0 create ${ENV_NAME}' first"
        exit 1
    fi

    echo "    Bucket: ${BUCKET_NAME}"
    echo "    Account: ${CF_ACCOUNT_ID}"
    echo ""

    # Service token is required for maintenance jobs
    if [[ -z "${SERVICE_TOKEN:-}" ]]; then
        echo "Error: Service token required for catalog maintenance"
        echo ""
        echo "Create an R2 API token at:"
        echo "  https://dash.cloudflare.com/?to=/:account/r2/api-tokens"
        echo ""
        echo "Required permissions:"
        echo "  - R2 storage: Admin Read & Write"
        echo "  - R2 Data Catalog: Read & Write"
        echo "  - Scope: ${BUCKET_NAME}"
        echo ""
        echo "Usage: $0 configure ${ENV_NAME} --token <R2_API_TOKEN>"
        exit 1
    fi

    # Set the service credential for maintenance jobs
    echo "==> Setting service credential for maintenance jobs..."
    local cred_response
    cred_response=$(curl -s -X POST \
        "https://api.cloudflare.com/client/v4/accounts/${CF_ACCOUNT_ID}/r2-catalog/${BUCKET_NAME}/credential" \
        -H "Authorization: Bearer ${CF_API_TOKEN}" \
        -H "Content-Type: application/json" \
        -d "{\"token\": \"${SERVICE_TOKEN}\"}")

    if echo "$cred_response" | jq -e '.success == true' >/dev/null 2>&1; then
        echo "    Service credential: set"
    else
        echo "Error: Failed to set service credential"
        if echo "$cred_response" | jq -e '.errors' >/dev/null 2>&1; then
            echo "       $(echo "$cred_response" | jq -r '.errors[0].message // "Unknown error"')"
        else
            echo "       Response: $cred_response"
        fi
        exit 1
    fi
    echo ""

    # Configure maintenance settings via REST API
    # Note: API uses max_snapshot_age (Go duration) and min_snapshots_to_keep
    echo "==> Enabling maintenance settings..."
    local response
    response=$(curl -s -X POST \
        "https://api.cloudflare.com/client/v4/accounts/${CF_ACCOUNT_ID}/r2-catalog/${BUCKET_NAME}/maintenance-configs" \
        -H "Authorization: Bearer ${CF_API_TOKEN}" \
        -H "Content-Type: application/json" \
        -d '{
            "compaction": {"state": "enabled"},
            "snapshot_expiration": {"state": "enabled", "max_snapshot_age": "1d", "min_snapshots_to_keep": 1}
        }')

    # Check if response is valid JSON and successful
    if echo "$response" | jq -e '.success == true' >/dev/null 2>&1; then
        echo "    Compaction: enabled"
        echo "    Snapshot expiration: enabled (max_snapshot_age=1d, min_snapshots_to_keep=1)"
        echo ""
        echo "==> Catalog maintenance configured successfully"
    else
        echo "Error: Failed to configure catalog maintenance"
        if echo "$response" | jq -e '.errors' >/dev/null 2>&1; then
            echo "       $(echo "$response" | jq -r '.errors[0].message // "Unknown error"')"
        else
            echo "       Response: $response"
        fi
        exit 1
    fi
}

case "$ACTION" in
    create)
        create_env
        ;;
    delete)
        delete_env
        ;;
    status)
        status_env
        ;;
    dry-run)
        dry_run
        ;;
    query)
        query_env
        ;;
    configure)
        configure_catalog
        ;;
    *)
        echo "Unknown action: $ACTION"
        echo "Use: create, delete, status, dry-run, query, or configure"
        exit 1
        ;;
esac
