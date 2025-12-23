use bytes::Bytes;
use std::sync::Arc;
use vrl::value::{ObjectMap, Value as VrlValue};

/// Precomputed fields for building a gauge metric record into VRL values
pub struct GaugeRecordParts {
    pub time_unix_nano: i64,
    pub start_time_unix_nano: i64,
    pub metric_name: Bytes,
    pub metric_description: Bytes,
    pub metric_unit: Bytes,
    pub value: VrlValue,
    pub attributes: VrlValue,
    pub resource: Arc<VrlValue>,
    pub scope: Arc<VrlValue>,
    pub flags: i64,
    pub exemplars: Vec<ExemplarParts>,
}

pub struct ExemplarParts {
    pub time_unix_nano: i64,
    pub value: VrlValue,
    pub trace_id: Bytes,
    pub span_id: Bytes,
    pub filtered_attributes: VrlValue,
}

/// Precomputed fields for building a sum metric record into VRL values
pub struct SumRecordParts {
    pub time_unix_nano: i64,
    pub start_time_unix_nano: i64,
    pub metric_name: Bytes,
    pub metric_description: Bytes,
    pub metric_unit: Bytes,
    pub value: VrlValue,
    pub attributes: VrlValue,
    pub resource: Arc<VrlValue>,
    pub scope: Arc<VrlValue>,
    pub flags: i64,
    pub exemplars: Vec<ExemplarParts>,
    pub aggregation_temporality: i64,
    pub is_monotonic: bool,
}

/// Pre-allocate values Vec for metrics
pub fn preallocate_metric_values<R, F>(resource_metrics: &[R], count_points: F) -> Vec<VrlValue>
where
    F: Fn(&R) -> usize,
{
    let capacity: usize = resource_metrics.iter().map(&count_points).sum();
    Vec::with_capacity(capacity)
}

/// Helper function to build exemplars array from parts
fn build_exemplars(exemplars: Vec<ExemplarParts>) -> VrlValue {
    let exemplars_array: Vec<VrlValue> = exemplars
        .into_iter()
        .map(|e| {
            let mut map = ObjectMap::new();
            map.insert("time_unix_nano".into(), VrlValue::Integer(e.time_unix_nano));
            map.insert("value".into(), e.value);
            map.insert("trace_id".into(), VrlValue::Bytes(e.trace_id));
            map.insert("span_id".into(), VrlValue::Bytes(e.span_id));
            map.insert("filtered_attributes".into(), e.filtered_attributes);
            VrlValue::Object(map)
        })
        .collect();
    VrlValue::Array(exemplars_array)
}

pub fn build_gauge_record(parts: GaugeRecordParts) -> VrlValue {
    // Debug assertions to catch schema violations early
    debug_assert!(
        parts.time_unix_nano >= 0,
        "gauge timestamp must be non-negative"
    );
    debug_assert!(
        matches!(parts.value, VrlValue::Float(_)),
        "gauge value must be a float, got: {:?}",
        parts.value
    );

    let mut map = ObjectMap::new();
    map.insert(
        "time_unix_nano".into(),
        VrlValue::Integer(parts.time_unix_nano),
    );
    map.insert(
        "start_time_unix_nano".into(),
        VrlValue::Integer(parts.start_time_unix_nano),
    );
    map.insert("metric_name".into(), VrlValue::Bytes(parts.metric_name));
    map.insert(
        "metric_description".into(),
        VrlValue::Bytes(parts.metric_description),
    );
    map.insert("metric_unit".into(), VrlValue::Bytes(parts.metric_unit));
    map.insert("value".into(), parts.value);
    map.insert("attributes".into(), parts.attributes);
    map.insert("resource".into(), (*parts.resource).clone());
    map.insert("scope".into(), (*parts.scope).clone());
    map.insert("flags".into(), VrlValue::Integer(parts.flags));
    map.insert("exemplars".into(), build_exemplars(parts.exemplars));
    map.insert("_metric_type".into(), VrlValue::Bytes(Bytes::from("gauge")));
    VrlValue::Object(map)
}

pub fn build_sum_record(parts: SumRecordParts) -> VrlValue {
    // Debug assertions to catch schema violations early
    debug_assert!(
        parts.time_unix_nano >= 0,
        "sum timestamp must be non-negative"
    );
    debug_assert!(
        matches!(parts.value, VrlValue::Float(_)),
        "sum value must be a float, got: {:?}",
        parts.value
    );

    let mut map = ObjectMap::new();
    map.insert(
        "time_unix_nano".into(),
        VrlValue::Integer(parts.time_unix_nano),
    );
    map.insert(
        "start_time_unix_nano".into(),
        VrlValue::Integer(parts.start_time_unix_nano),
    );
    map.insert("metric_name".into(), VrlValue::Bytes(parts.metric_name));
    map.insert(
        "metric_description".into(),
        VrlValue::Bytes(parts.metric_description),
    );
    map.insert("metric_unit".into(), VrlValue::Bytes(parts.metric_unit));
    map.insert("value".into(), parts.value);
    map.insert("attributes".into(), parts.attributes);
    map.insert("resource".into(), (*parts.resource).clone());
    map.insert("scope".into(), (*parts.scope).clone());
    map.insert("flags".into(), VrlValue::Integer(parts.flags));
    map.insert("exemplars".into(), build_exemplars(parts.exemplars));
    map.insert(
        "aggregation_temporality".into(),
        VrlValue::Integer(parts.aggregation_temporality),
    );
    map.insert("is_monotonic".into(), VrlValue::Boolean(parts.is_monotonic));
    map.insert("_metric_type".into(), VrlValue::Bytes(Bytes::from("sum")));
    VrlValue::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_gauge_record_from_parts() {
        let parts = GaugeRecordParts {
            time_unix_nano: 1000000,
            start_time_unix_nano: 900000,
            metric_name: Bytes::from("cpu.usage"),
            metric_description: Bytes::from("CPU usage percentage"),
            metric_unit: Bytes::from("1"),
            value: VrlValue::Float(ordered_float::NotNan::new(0.75).unwrap()),
            attributes: VrlValue::Object(ObjectMap::new()),
            resource: Arc::new(VrlValue::Object(ObjectMap::new())),
            scope: Arc::new(VrlValue::Object(ObjectMap::new())),
            flags: 0,
            exemplars: vec![],
        };

        let record = build_gauge_record(parts);

        let obj = match record {
            VrlValue::Object(map) => map,
            _ => panic!("expected object"),
        };

        assert_eq!(obj.get("time_unix_nano"), Some(&VrlValue::Integer(1000000)));
        assert_eq!(
            obj.get("metric_name"),
            Some(&VrlValue::Bytes(Bytes::from("cpu.usage")))
        );
        assert_eq!(
            obj.get("_metric_type"),
            Some(&VrlValue::Bytes(Bytes::from("gauge")))
        );
    }

    #[test]
    fn builds_sum_record_from_parts() {
        let parts = SumRecordParts {
            time_unix_nano: 1000000,
            start_time_unix_nano: 900000,
            metric_name: Bytes::from("requests.count"),
            metric_description: Bytes::from("Total requests"),
            metric_unit: Bytes::from("1"),
            value: VrlValue::Float(ordered_float::NotNan::new(42.0).unwrap()),
            attributes: VrlValue::Object(ObjectMap::new()),
            resource: Arc::new(VrlValue::Object(ObjectMap::new())),
            scope: Arc::new(VrlValue::Object(ObjectMap::new())),
            flags: 0,
            exemplars: vec![],
            aggregation_temporality: 2,
            is_monotonic: true,
        };

        let record = build_sum_record(parts);

        let obj = match record {
            VrlValue::Object(map) => map,
            _ => panic!("expected object"),
        };

        assert_eq!(obj.get("time_unix_nano"), Some(&VrlValue::Integer(1000000)));
        assert_eq!(
            obj.get("metric_name"),
            Some(&VrlValue::Bytes(Bytes::from("requests.count")))
        );
        assert_eq!(
            obj.get("_metric_type"),
            Some(&VrlValue::Bytes(Bytes::from("sum")))
        );
        assert_eq!(
            obj.get("aggregation_temporality"),
            Some(&VrlValue::Integer(2))
        );
        assert_eq!(obj.get("is_monotonic"), Some(&VrlValue::Boolean(true)));
    }

    #[test]
    fn builds_exemplars_correctly() {
        let parts = GaugeRecordParts {
            time_unix_nano: 1000000,
            start_time_unix_nano: 900000,
            metric_name: Bytes::from("cpu.usage"),
            metric_description: Bytes::from(""),
            metric_unit: Bytes::from("1"),
            value: VrlValue::Float(ordered_float::NotNan::new(0.75).unwrap()),
            attributes: VrlValue::Object(ObjectMap::new()),
            resource: Arc::new(VrlValue::Object(ObjectMap::new())),
            scope: Arc::new(VrlValue::Object(ObjectMap::new())),
            flags: 0,
            exemplars: vec![ExemplarParts {
                time_unix_nano: 1500000,
                value: VrlValue::Float(ordered_float::NotNan::new(0.8).unwrap()),
                trace_id: Bytes::from("trace123"),
                span_id: Bytes::from("span456"),
                filtered_attributes: VrlValue::Object(ObjectMap::new()),
            }],
        };

        let record = build_gauge_record(parts);

        let obj = match record {
            VrlValue::Object(map) => map,
            _ => panic!("expected object"),
        };

        let exemplars = obj.get("exemplars").unwrap();
        match exemplars {
            VrlValue::Array(arr) => {
                assert_eq!(arr.len(), 1);
                match &arr[0] {
                    VrlValue::Object(exemplar_map) => {
                        assert_eq!(
                            exemplar_map.get("time_unix_nano"),
                            Some(&VrlValue::Integer(1500000))
                        );
                        assert_eq!(
                            exemplar_map.get("trace_id"),
                            Some(&VrlValue::Bytes(Bytes::from("trace123")))
                        );
                    }
                    _ => panic!("expected exemplar object"),
                }
            }
            _ => panic!("expected exemplars array"),
        }
    }
}
