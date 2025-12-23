// src/transform/runtime.rs
use once_cell::sync::Lazy;
use std::collections::HashMap;
use vrl::compiler::runtime::Runtime;
use vrl::compiler::{compile, Program, TargetValue, TimeZone};
use vrl::value::{KeyString, Value};

static UTC_TIMEZONE: Lazy<TimeZone> = Lazy::new(|| TimeZone::Named(chrono_tz::UTC));

use super::functions;

// Include compiled VRL sources from build.rs
include!(concat!(env!("OUT_DIR"), "/compiled_vrl.rs"));

// Lazy-compiled programs with our custom functions
pub static OTLP_LOGS_PROGRAM: Lazy<Program> = Lazy::new(|| {
    let fns = functions::all();
    compile(OTLP_LOGS_SOURCE, &fns)
        .expect("OTLP_LOGS VRL should compile")
        .program
});

pub static OTLP_TRACES_PROGRAM: Lazy<Program> = Lazy::new(|| {
    let fns = functions::all();
    compile(OTLP_TRACES_SOURCE, &fns)
        .expect("OTLP_TRACES VRL should compile")
        .program
});

pub static OTLP_GAUGE_PROGRAM: Lazy<Program> = Lazy::new(|| {
    let fns = functions::all();
    compile(OTLP_GAUGE_SOURCE, &fns)
        .expect("OTLP_GAUGE VRL should compile")
        .program
});

pub static OTLP_SUM_PROGRAM: Lazy<Program> = Lazy::new(|| {
    let fns = functions::all();
    compile(OTLP_SUM_SOURCE, &fns)
        .expect("OTLP_SUM VRL should compile")
        .program
});

pub static HEC_LOGS_PROGRAM: Lazy<Program> = Lazy::new(|| {
    let fns = functions::all();
    compile(HEC_LOGS_SOURCE, &fns)
        .expect("HEC_LOGS VRL should compile")
        .program
});

pub struct VrlTransformer {
    runtime: Runtime,
}

#[derive(Debug)]
pub struct VrlError(pub String);

impl std::fmt::Display for VrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VRL error: {}", self.0)
    }
}

impl VrlTransformer {
    pub fn new() -> Self {
        Self {
            runtime: Runtime::default(),
        }
    }

    pub fn transform(
        &mut self,
        program: &Program,
        input: Value,
    ) -> Result<(String, Value), VrlError> {
        let mut target = TargetValue {
            value: input,
            metadata: Value::Object(Default::default()),
            secrets: Default::default(),
        };

        self.runtime
            .resolve(&mut target, program, &UTC_TIMEZONE)
            .map_err(|e| VrlError(format!("{:?}", e)))?;

        let table_key: KeyString = "_table".into();
        let table = if let Value::Object(ref map) = target.value {
            map.get(&table_key)
                .and_then(|v| match v {
                    Value::Bytes(b) => Some(String::from_utf8_lossy(b).to_string()),
                    _ => None,
                })
                .unwrap_or_else(|| "unknown".to_string())
        } else {
            "unknown".to_string()
        };

        if let Value::Object(ref mut map) = target.value {
            map.remove(&table_key);
        }

        Ok((table, target.value))
    }

    pub fn transform_batch(
        &mut self,
        program: &Program,
        inputs: Vec<Value>,
    ) -> Result<HashMap<String, Vec<Value>>, VrlError> {
        let mut grouped: HashMap<String, Vec<Value>> = HashMap::new();

        for (idx, input) in inputs.into_iter().enumerate() {
            let (table, output) = self
                .transform(program, input)
                .map_err(|e| VrlError(format!("record {}: {}", idx, e.0)))?;
            grouped.entry(table).or_default().push(output);
        }

        Ok(grouped)
    }
}

/// Force initialization of all VRL programs.
/// Call during worker startup to avoid cold-start latency.
#[cfg(target_arch = "wasm32")]
pub fn init_programs() {
    // Access each Lazy to force initialization
    let _ = &*OTLP_LOGS_PROGRAM;
    let _ = &*OTLP_TRACES_PROGRAM;
    let _ = &*OTLP_GAUGE_PROGRAM;
    let _ = &*OTLP_SUM_PROGRAM;
    let _ = &*HEC_LOGS_PROGRAM;
}

impl Default for VrlTransformer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use ordered_float::NotNan;
    use vrl::value::ObjectMap;

    #[test]
    fn gauge_vrl_sets_service_name_unknown_when_missing() {
        // Build a gauge record without service.name in resource.attributes
        let mut resource_attrs = ObjectMap::new();
        resource_attrs.insert(
            "host.name".into(),
            Value::Bytes(Bytes::from("docker-desktop")),
        );
        resource_attrs.insert("os.type".into(), Value::Bytes(Bytes::from("linux")));

        let mut resource = ObjectMap::new();
        resource.insert("attributes".into(), Value::Object(resource_attrs));

        let mut scope = ObjectMap::new();
        scope.insert("name".into(), Value::Bytes(Bytes::from("test.receiver")));
        scope.insert("version".into(), Value::Bytes(Bytes::from("1.0.0")));
        scope.insert("attributes".into(), Value::Object(ObjectMap::new()));

        let mut record = ObjectMap::new();
        record.insert("time_unix_nano".into(), Value::Integer(1766729681000000000));
        record.insert(
            "start_time_unix_nano".into(),
            Value::Integer(1766703548000000000),
        );
        record.insert(
            "metric_name".into(),
            Value::Bytes(Bytes::from("redis.clients.max_input_buffer")),
        );
        record.insert(
            "metric_description".into(),
            Value::Bytes(Bytes::from("Biggest input buffer")),
        );
        record.insert("metric_unit".into(), Value::Bytes(Bytes::from("By")));
        record.insert("value".into(), Value::Float(NotNan::new(0.0).unwrap()));
        record.insert("attributes".into(), Value::Object(ObjectMap::new()));
        record.insert("resource".into(), Value::Object(resource));
        record.insert("scope".into(), Value::Object(scope));
        record.insert("flags".into(), Value::Integer(0));
        record.insert("exemplars".into(), Value::Array(vec![]));
        record.insert("_metric_type".into(), Value::Bytes(Bytes::from("gauge")));

        let input = Value::Object(record);

        let mut transformer = VrlTransformer::new();
        let result = transformer.transform(&OTLP_GAUGE_PROGRAM, input);

        assert!(result.is_ok(), "VRL transformation should succeed");
        let (table, output) = result.unwrap();
        assert_eq!(table, "gauge");

        if let Value::Object(map) = output {
            // Check service_name is set to "unknown"
            let key: KeyString = "service_name".into();
            let service_name = map.get(&key);
            assert!(
                service_name.is_some(),
                "service_name should be present in output"
            );
            assert_eq!(
                service_name.unwrap(),
                &Value::Bytes(Bytes::from("unknown")),
                "service_name should be 'unknown' when service.name is missing"
            );
        } else {
            panic!("Expected Object output");
        }
    }
}
