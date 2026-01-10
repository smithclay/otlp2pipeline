#!/bin/bash
#
# Send a test record to Firehose to verify end-to-end delivery
#
# Usage:
#   ./scripts/aws-send-test-record.sh <stream-name> [region]
#   ./scripts/aws-send-test-record.sh otlp2pipeline-prod us-east-1
#

set -e

STREAM_NAME="${1:?Usage: $0 <stream-name> [region]}"
REGION="${2:-us-east-1}"
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
OBSERVED_TS=$(date +%s)000

echo "==> Sending test record to Firehose"
echo "    Stream: ${STREAM_NAME}"
echo "    Region: ${REGION}"
echo "    Timestamp: ${TIMESTAMP}"

# Create test record matching OTLP logs schema
TEST_RECORD=$(cat <<EOF
{
  "timestamp": "${TIMESTAMP}",
  "observed_timestamp": ${OBSERVED_TS},
  "service_name": "test-service",
  "severity_number": 9,
  "severity_text": "INFO",
  "body": "Test message sent at ${TIMESTAMP}",
  "trace_id": "",
  "span_id": "",
  "flags": 0,
  "resource_attributes": "{}",
  "log_attributes": "{}",
  "scope_name": "",
  "scope_version": "",
  "scope_attributes": "{}"
}
EOF
)

# Base64 encode for Firehose
ENCODED=$(echo "${TEST_RECORD}" | base64)

# Create records file
RECORDS_FILE=$(mktemp)
echo "[{\"Data\":\"${ENCODED}\"}]" > "${RECORDS_FILE}"

# Send to Firehose
echo ""
echo "==> Sending record..."
RESULT=$(aws firehose put-record-batch \
    --delivery-stream-name "${STREAM_NAME}" \
    --region "${REGION}" \
    --records "file://${RECORDS_FILE}" 2>&1)

rm -f "${RECORDS_FILE}"

FAILED_COUNT=$(echo "${RESULT}" | grep -o '"FailedPutCount": [0-9]*' | grep -o '[0-9]*')

if [ "${FAILED_COUNT}" = "0" ]; then
    echo "    Success! Record accepted by Firehose."
    echo ""
    echo "==> Checking delivery metrics (wait ~60s for buffering)..."
    echo "    Run this to check delivery status:"
    echo ""
    echo "    aws cloudwatch get-metric-statistics \\"
    echo "      --namespace AWS/Firehose \\"
    echo "      --metric-name DeliveryToIceberg.SuccessfulRowCount \\"
    echo "      --dimensions Name=DeliveryStreamName,Value=${STREAM_NAME} \\"
    echo "      --start-time \$(date -u -v-5M '+%Y-%m-%dT%H:%M:%SZ') \\"
    echo "      --end-time \$(date -u '+%Y-%m-%dT%H:%M:%SZ') \\"
    echo "      --period 60 --statistics Sum --region ${REGION}"
else
    echo "    ERROR: ${FAILED_COUNT} record(s) failed"
    echo "${RESULT}"
    exit 1
fi
