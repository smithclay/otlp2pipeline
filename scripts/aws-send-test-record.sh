#!/bin/bash
#
# Send test records to Firehose to verify end-to-end delivery
#
# Usage:
#   ./scripts/aws-send-test-record.sh <stack-name> [region] [signal]
#   ./scripts/aws-send-test-record.sh otlp2pipeline-prod us-east-1        # both logs and traces
#   ./scripts/aws-send-test-record.sh otlp2pipeline-prod us-east-1 logs   # logs only
#   ./scripts/aws-send-test-record.sh otlp2pipeline-prod us-east-1 traces # traces only
#

set -e

STACK_NAME="${1:?Usage: $0 <stack-name> [region] [signal]}"
REGION="${2:-us-east-1}"
SIGNAL="${3:-both}"  # logs, traces, or both
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
OBSERVED_TS=$(date +%s)000
END_TS=$((OBSERVED_TS + 100))
TRACE_ID="0123456789abcdef0123456789abcdef"
SPAN_ID="0123456789abcdef"

send_record() {
    local stream_name="$1"
    local record="$2"
    local signal_type="$3"

    local encoded=$(echo "${record}" | base64)
    local records_file=$(mktemp)
    echo "[{\"Data\":\"${encoded}\"}]" > "${records_file}"

    echo ""
    echo "==> Sending ${signal_type} record to ${stream_name}..."
    local result=$(aws firehose put-record-batch \
        --delivery-stream-name "${stream_name}" \
        --region "${REGION}" \
        --records "file://${records_file}" 2>&1)

    rm -f "${records_file}"

    local failed_count=$(echo "${result}" | grep -o '"FailedPutCount": [0-9]*' | grep -o '[0-9]*')

    if [ "${failed_count}" = "0" ]; then
        echo "    Success! ${signal_type} record accepted by Firehose."
    else
        echo "    ERROR: ${failed_count} record(s) failed"
        echo "${result}"
        return 1
    fi
}

echo "==> Sending test records to Firehose"
echo "    Stack: ${STACK_NAME}"
echo "    Region: ${REGION}"
echo "    Signal: ${SIGNAL}"
echo "    Timestamp: ${TIMESTAMP}"

# Send logs record
if [ "${SIGNAL}" = "logs" ] || [ "${SIGNAL}" = "both" ]; then
    LOG_RECORD=$(cat <<EOF
{
  "timestamp": "${TIMESTAMP}",
  "observed_timestamp": ${OBSERVED_TS},
  "service_name": "test-service",
  "severity_number": 9,
  "severity_text": "INFO",
  "body": "Test log message at ${TIMESTAMP}",
  "trace_id": "${TRACE_ID}",
  "span_id": "${SPAN_ID}",
  "resource_attributes": "{\"host.name\":\"test-host\"}",
  "log_attributes": "{\"test\":true}",
  "scope_name": "test-scope",
  "scope_version": "1.0.0",
  "scope_attributes": "{}"
}
EOF
)
    send_record "${STACK_NAME}-logs" "${LOG_RECORD}" "logs"
fi

# Send traces record
if [ "${SIGNAL}" = "traces" ] || [ "${SIGNAL}" = "both" ]; then
    TRACE_RECORD=$(cat <<EOF
{
  "timestamp": "${TIMESTAMP}",
  "end_timestamp": ${END_TS},
  "duration": 100,
  "trace_id": "${TRACE_ID}",
  "span_id": "${SPAN_ID}",
  "parent_span_id": "",
  "trace_state": "",
  "service_name": "test-service",
  "service_namespace": "",
  "service_instance_id": "",
  "span_name": "test-operation",
  "span_kind": 1,
  "status_code": 0,
  "status_message": "",
  "resource_attributes": "{\"host.name\":\"test-host\"}",
  "scope_name": "test-scope",
  "scope_version": "1.0.0",
  "scope_attributes": "{}",
  "span_attributes": "{\"test\":true}",
  "events_json": "[]",
  "links_json": "[]",
  "dropped_attributes_count": 0,
  "dropped_events_count": 0,
  "dropped_links_count": 0,
  "flags": 0
}
EOF
)
    send_record "${STACK_NAME}-traces" "${TRACE_RECORD}" "traces"
fi

echo ""
echo "==> Records sent! Data will appear in tables after Firehose buffering (~2 min)."
echo ""
echo "    Query logs:"
echo "    aws s3tables query --table-bucket-arn <bucket-arn> --namespace default --name logs --region ${REGION}"
echo ""
echo "    Query traces:"
echo "    aws s3tables query --table-bucket-arn <bucket-arn> --namespace default --name traces --region ${REGION}"
