use bytes::Bytes;
use std::collections::HashMap;
use vrl::value::Value;

use crate::decode::{hec, otlp, DecodeFormat};
use crate::signal::Signal;
use crate::transform::runtime::{
    HEC_LOGS_PROGRAM, OTLP_GAUGE_PROGRAM, OTLP_LOGS_PROGRAM, OTLP_SUM_PROGRAM, OTLP_TRACES_PROGRAM,
};
use crate::transform::{VrlError, VrlTransformer};

use super::{DecodeError, SignalHandler};

/// Handler for OTLP logs
pub struct LogsHandler;

impl SignalHandler for LogsHandler {
    const SIGNAL: Signal = Signal::Logs;

    fn decode(body: Bytes, format: DecodeFormat) -> Result<Vec<Value>, DecodeError> {
        otlp::decode_logs(body, format).map_err(|e| DecodeError(e.to_string()))
    }

    fn vrl_program() -> &'static vrl::compiler::Program {
        &OTLP_LOGS_PROGRAM
    }
}

/// Handler for OTLP traces
pub struct TracesHandler;

impl SignalHandler for TracesHandler {
    const SIGNAL: Signal = Signal::Traces;

    fn decode(body: Bytes, format: DecodeFormat) -> Result<Vec<Value>, DecodeError> {
        otlp::decode_traces(body, format).map_err(|e| DecodeError(e.to_string()))
    }

    fn vrl_program() -> &'static vrl::compiler::Program {
        &OTLP_TRACES_PROGRAM
    }
}

/// Handler for Splunk HEC logs
pub struct HecLogsHandler;

impl SignalHandler for HecLogsHandler {
    const SIGNAL: Signal = Signal::Logs;

    fn decode(body: Bytes, _format: DecodeFormat) -> Result<Vec<Value>, DecodeError> {
        hec::decode_hec_logs(body).map_err(|e| DecodeError(e.to_string()))
    }

    fn vrl_program() -> &'static vrl::compiler::Program {
        &HEC_LOGS_PROGRAM
    }
}

/// Handler for OTLP metrics (gauge and sum)
pub struct MetricsHandler;

impl MetricsHandler {
    fn partition_by_type(values: Vec<Value>) -> (Vec<Value>, Vec<Value>) {
        let mut gauges = Vec::new();
        let mut sums = Vec::new();

        for value in values {
            if let Value::Object(ref map) = value {
                if let Some(Value::Bytes(b)) = map.get("_metric_type") {
                    match b.as_ref() {
                        b"gauge" => gauges.push(value),
                        b"sum" => sums.push(value),
                        _ => {}
                    }
                }
            }
        }

        (gauges, sums)
    }
}

impl SignalHandler for MetricsHandler {
    const SIGNAL: Signal = Signal::Gauge;

    fn decode(body: Bytes, format: DecodeFormat) -> Result<Vec<Value>, DecodeError> {
        otlp::decode_metrics(body, format).map_err(|e| DecodeError(e.to_string()))
    }

    fn vrl_program() -> &'static vrl::compiler::Program {
        &OTLP_GAUGE_PROGRAM
    }

    fn transform_batch(
        transformer: &mut VrlTransformer,
        values: Vec<Value>,
    ) -> Result<HashMap<String, Vec<Value>>, VrlError> {
        let (gauges, sums) = Self::partition_by_type(values);
        let mut grouped: HashMap<String, Vec<Value>> = HashMap::new();

        if !gauges.is_empty() {
            let gauge_grouped = transformer.transform_batch(&OTLP_GAUGE_PROGRAM, gauges)?;
            for (table, records) in gauge_grouped {
                grouped.entry(table).or_default().extend(records);
            }
        }

        if !sums.is_empty() {
            let sum_grouped = transformer.transform_batch(&OTLP_SUM_PROGRAM, sums)?;
            for (table, records) in sum_grouped {
                grouped.entry(table).or_default().extend(records);
            }
        }

        Ok(grouped)
    }
}
