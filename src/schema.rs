//! Schema definitions for pipeline output validation.
//!
//! These schemas mirror the @schema definitions from otlp2records.
//! Keep them in sync; tests validate parity with otlp2records schema defs.

use serde_json::Value as JsonValue;

/// Field type for schema validation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Timestamp, // number (milliseconds)
    Int32,
    Int64,
    Float64,
    String,
    Bool,
    #[allow(dead_code)] // Reserved for future use
    Json,
}

impl FieldType {
    /// Check if a JSON value matches this field type
    pub fn matches(&self, value: &JsonValue) -> bool {
        match self {
            FieldType::Timestamp | FieldType::Int32 | FieldType::Int64 => value.is_number(),
            FieldType::Float64 => value.is_f64(),
            FieldType::String => value.is_string(),
            FieldType::Bool => value.is_boolean(),
            FieldType::Json => value.is_object() || value.is_array() || value.is_string(),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            FieldType::Timestamp => "timestamp (number)",
            FieldType::Int32 => "int32",
            FieldType::Int64 => "int64",
            FieldType::Float64 => "float64",
            FieldType::String => "string",
            FieldType::Bool => "bool",
            FieldType::Json => "json",
        }
    }
}

/// A required field in a schema
#[derive(Debug, Clone)]
pub struct RequiredField {
    pub name: &'static str,
    pub field_type: FieldType,
}

/// Schema for a signal type
#[derive(Debug)]
pub struct Schema {
    pub name: &'static str,
    pub required_fields: &'static [RequiredField],
}

impl Schema {
    /// Validate a JSON record against this schema
    pub fn validate(&self, json: &JsonValue, record_idx: usize) -> Result<(), String> {
        let obj = json.as_object().ok_or_else(|| {
            format!(
                "record {} ({}): expected object, got {}",
                record_idx,
                self.name,
                json_type_name(json)
            )
        })?;

        for field in self.required_fields {
            match obj.get(field.name) {
                None => {
                    return Err(format!(
                        "record {} ({}): missing required field '{}'. Record: {}",
                        record_idx,
                        self.name,
                        field.name,
                        truncate_json(json, 500)
                    ));
                }
                Some(value) if !field.field_type.matches(value) => {
                    return Err(format!(
                        "record {} ({}): field '{}' has wrong type, expected {}, got {}. Record: {}",
                        record_idx,
                        self.name,
                        field.name,
                        field.field_type.name(),
                        json_type_name(value),
                        truncate_json(json, 500)
                    ));
                }
                Some(_) => {} // Valid
            }
        }

        Ok(())
    }
}

fn json_type_name(v: &JsonValue) -> &'static str {
    match v {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "bool",
        JsonValue::Number(n) if n.is_f64() => "float64",
        JsonValue::Number(_) => "integer",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

fn truncate_json(json: &JsonValue, max_len: usize) -> String {
    let s = serde_json::to_string(json).unwrap_or_else(|_| "<serialize failed>".into());
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s
    }
}

// ============================================================================
// Schema Definitions - keep in sync with otlp2records schema definitions
// ============================================================================

/// Gauge metrics schema (otlp2records schema: gauge)
pub static GAUGE_SCHEMA: Schema = Schema {
    name: "gauge",
    required_fields: &[
        RequiredField {
            name: "timestamp",
            field_type: FieldType::Timestamp,
        },
        RequiredField {
            name: "metric_name",
            field_type: FieldType::String,
        },
        RequiredField {
            name: "value",
            field_type: FieldType::Float64,
        },
        RequiredField {
            name: "service_name",
            field_type: FieldType::String,
        },
    ],
};

/// Sum metrics schema (otlp2records schema: sum)
pub static SUM_SCHEMA: Schema = Schema {
    name: "sum",
    required_fields: &[
        RequiredField {
            name: "timestamp",
            field_type: FieldType::Timestamp,
        },
        RequiredField {
            name: "metric_name",
            field_type: FieldType::String,
        },
        RequiredField {
            name: "value",
            field_type: FieldType::Float64,
        },
        RequiredField {
            name: "service_name",
            field_type: FieldType::String,
        },
        RequiredField {
            name: "aggregation_temporality",
            field_type: FieldType::Int32,
        },
        RequiredField {
            name: "is_monotonic",
            field_type: FieldType::Bool,
        },
    ],
};

/// Logs schema (otlp2records schema: logs)
pub static LOGS_SCHEMA: Schema = Schema {
    name: "logs",
    required_fields: &[
        RequiredField {
            name: "timestamp",
            field_type: FieldType::Timestamp,
        },
        RequiredField {
            name: "observed_timestamp",
            field_type: FieldType::Int64,
        },
        RequiredField {
            name: "service_name",
            field_type: FieldType::String,
        },
        RequiredField {
            name: "severity_number",
            field_type: FieldType::Int32,
        },
        RequiredField {
            name: "severity_text",
            field_type: FieldType::String,
        },
    ],
};

/// Traces/spans schema (otlp2records schema: spans)
pub static TRACES_SCHEMA: Schema = Schema {
    name: "traces",
    required_fields: &[
        RequiredField {
            name: "timestamp",
            field_type: FieldType::Timestamp,
        },
        RequiredField {
            name: "end_timestamp",
            field_type: FieldType::Int64,
        },
        RequiredField {
            name: "duration",
            field_type: FieldType::Int64,
        },
        RequiredField {
            name: "service_name",
            field_type: FieldType::String,
        },
        RequiredField {
            name: "span_name",
            field_type: FieldType::String,
        },
        RequiredField {
            name: "span_kind",
            field_type: FieldType::Int32,
        },
        RequiredField {
            name: "status_code",
            field_type: FieldType::Int32,
        },
    ],
};

/// Get schema for a table name
pub fn get_schema(table: &str) -> Option<&'static Schema> {
    match table {
        "gauge" => Some(&GAUGE_SCHEMA),
        "sum" => Some(&SUM_SCHEMA),
        "logs" => Some(&LOGS_SCHEMA),
        "traces" => Some(&TRACES_SCHEMA),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn gauge_schema_validates_complete_record() {
        let record = json!({
            "timestamp": 1234567890,
            "metric_name": "cpu.usage",
            "value": 42.5,
            "service_name": "my-service"
        });
        assert!(GAUGE_SCHEMA.validate(&record, 0).is_ok());
    }

    #[test]
    fn gauge_schema_rejects_missing_value() {
        let record = json!({
            "timestamp": 1234567890,
            "metric_name": "cpu.usage",
            "service_name": "my-service"
        });
        let err = GAUGE_SCHEMA.validate(&record, 0).unwrap_err();
        assert!(err.contains("missing required field 'value'"));
    }

    #[test]
    fn gauge_schema_rejects_wrong_type() {
        let record = json!({
            "timestamp": 1234567890,
            "metric_name": "cpu.usage",
            "value": "not a float",
            "service_name": "my-service"
        });
        let err = GAUGE_SCHEMA.validate(&record, 0).unwrap_err();
        assert!(err.contains("wrong type"));
    }

    #[test]
    fn sum_schema_validates_complete_record() {
        let record = json!({
            "timestamp": 1234567890,
            "metric_name": "requests.total",
            "value": 100.0,
            "service_name": "my-service",
            "aggregation_temporality": 2,
            "is_monotonic": true
        });
        assert!(SUM_SCHEMA.validate(&record, 0).is_ok());
    }

    #[test]
    fn logs_schema_validates_complete_record() {
        let record = json!({
            "timestamp": 1234567890,
            "observed_timestamp": 1234567890,
            "service_name": "my-service",
            "severity_number": 9,
            "severity_text": "INFO"
        });
        assert!(LOGS_SCHEMA.validate(&record, 0).is_ok());
    }

    #[test]
    fn traces_schema_validates_complete_record() {
        let record = json!({
            "timestamp": 1234567890,
            "end_timestamp": 1234567891,
            "duration": 1,
            "trace_id": "abc123",
            "span_id": "def456",
            "service_name": "my-service",
            "span_name": "GET /api",
            "span_kind": 2,
            "status_code": 0
        });
        assert!(TRACES_SCHEMA.validate(&record, 0).is_ok());
    }

    #[test]
    fn get_schema_returns_correct_schemas() {
        assert!(get_schema("gauge").is_some());
        assert!(get_schema("sum").is_some());
        assert!(get_schema("logs").is_some());
        assert!(get_schema("traces").is_some());
        assert!(get_schema("unknown").is_none());
    }

    fn field_type_from_otlp2records(name: &str) -> Option<FieldType> {
        match name {
            "timestamp" => Some(FieldType::Timestamp),
            "int32" => Some(FieldType::Int32),
            "int64" => Some(FieldType::Int64),
            "float64" => Some(FieldType::Float64),
            "string" => Some(FieldType::String),
            "bool" => Some(FieldType::Bool),
            "json" => Some(FieldType::Json),
            _ => None,
        }
    }

    fn required_fields_map(schema: &Schema) -> HashMap<&'static str, FieldType> {
        schema
            .required_fields
            .iter()
            .map(|field| (field.name, field.field_type))
            .collect()
    }

    #[test]
    fn schema_defs_match_otlp2records() {
        let otlp_schemas: HashMap<_, _> = otlp2records::schema_defs()
            .iter()
            .map(|schema| (schema.name, schema))
            .collect();

        let local_schemas = [&GAUGE_SCHEMA, &SUM_SCHEMA, &LOGS_SCHEMA, &TRACES_SCHEMA];

        for local in local_schemas {
            let otlp_name = match local.name {
                "traces" => "spans",
                other => other,
            };
            let otlp_schema = otlp_schemas
                .get(otlp_name)
                .unwrap_or_else(|| panic!("missing otlp2records schema for {}", otlp_name));
            let local_fields = required_fields_map(local);
            let mut otlp_required = HashMap::new();

            for field in otlp_schema.fields {
                if field.required {
                    let field_type =
                        field_type_from_otlp2records(field.field_type).unwrap_or_else(|| {
                            panic!("unknown otlp2records field type {}", field.field_type)
                        });
                    otlp_required.insert(field.name, field_type);
                }
            }

            assert_eq!(
                local_fields, otlp_required,
                "required fields mismatch for schema {}",
                local.name
            );
        }
    }
}
