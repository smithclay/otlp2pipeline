// build.rs
use std::{collections::HashSet, env, fs, path::Path};

fn main() {
    compile_vrl_scripts();
}

// Schema field parsed from VRL annotations
struct SchemaField {
    name: String,
    field_type: String,
    required: bool,
}

// Schema parsed from VRL file header
struct Schema {
    name: String,
    fields: Vec<SchemaField>,
}

fn parse_schema_from_vrl(source: &str) -> Option<Schema> {
    let mut in_schema_block = false;
    let mut schema_name = None;
    let mut fields = Vec::new();

    for line in source.lines() {
        let line = line.trim();

        // Start of schema block
        if line.starts_with("# @schema ") {
            in_schema_block = true;
            schema_name = Some(line.trim_start_matches("# @schema ").trim().to_string());
            continue;
        }

        // End of schema block
        if line == "# @end" {
            break;
        }

        if !in_schema_block {
            continue;
        }

        // Skip @description and empty lines
        if line.starts_with("# @") || line == "#" || line.is_empty() {
            continue;
        }

        // Field definition: # field_name: type, required?, "description"?
        if line.starts_with("# ") && line.contains(':') {
            if let Some(field) = parse_field_line(&line[2..]) {
                fields.push(field);
            }
        }
    }

    schema_name.map(|name| Schema { name, fields })
}

fn parse_field_line(line: &str) -> Option<SchemaField> {
    // Format: field_name: type, required?, "description"?
    let mut parts = line.splitn(2, ':');
    let name = parts.next()?.trim().to_string();
    let rest = parts.next()?.trim();

    // Strip description in quotes if present (not used in Cloudflare schema)
    let rest = if let Some(quote_start) = rest.find('"') {
        rest[..quote_start].trim()
    } else {
        rest
    };

    // Parse type and required flag
    let mut field_type = String::new();
    let mut required = false;
    for part in rest.split(',') {
        let part = part.trim();
        if part == "required" {
            required = true;
        } else if !part.is_empty() && field_type.is_empty() {
            field_type = part.to_string();
        }
    }

    if field_type.is_empty() {
        return None;
    }

    Some(SchemaField {
        name,
        field_type,
        required,
    })
}

fn generate_cloudflare_schema(schema: &Schema) -> String {
    let mut fields_json = Vec::new();

    for field in &schema.fields {
        let mut field_obj = format!(
            r#"    {{ "name": "{}", "type": "{}", "required": {}"#,
            field.name, field.field_type, field.required
        );
        field_obj.push_str(" }");
        fields_json.push(field_obj);
    }

    format!(
        "{{\n  \"fields\": [\n{}\n  ]\n}}\n",
        fields_json.join(",\n")
    )
}

fn generate_arrow_schema(schema: &Schema) -> String {
    let mut fields = Vec::new();

    for field in &schema.fields {
        let (arrow_type, metadata) = match field.field_type.as_str() {
            "timestamp" => (
                "DataType::Int64",
                Some(r#"HashMap::from([("unit".to_string(), "MILLISECOND".to_string())])"#),
            ),
            "int64" => ("DataType::Int64", None),
            "int32" => ("DataType::Int32", None),
            "float64" => ("DataType::Float64", None),
            "bool" => ("DataType::Boolean", None),
            "string" => ("DataType::Utf8", None),
            "json" => (
                "DataType::Utf8",
                Some(r#"HashMap::from([("format".to_string(), "JSON".to_string())])"#),
            ),
            t => panic!("Unknown type in schema: {}", t),
        };

        let nullable = !field.required;

        let field_code = if let Some(meta) = metadata {
            format!(
                r#"        Field::new("{}", {}, {}).with_metadata({})"#,
                field.name, arrow_type, nullable, meta
            )
        } else {
            format!(
                r#"        Field::new("{}", {}, {})"#,
                field.name, arrow_type, nullable
            )
        };

        fields.push(field_code);
    }

    format!(
        r#"pub fn {}_schema() -> Schema {{
    Schema::new(vec![
{},
    ])
}}"#,
        schema.name,
        fields.join(",\n")
    )
}

fn generate_sqlite_ddl(schema: &Schema) -> String {
    let table_name = if schema.name == "spans" {
        "traces"
    } else {
        &schema.name
    };

    let mut columns = vec!["id INTEGER PRIMARY KEY AUTOINCREMENT".to_string()];

    for field in &schema.fields {
        let sql_type = match field.field_type.as_str() {
            "timestamp" | "int64" | "int32" => "INTEGER",
            "float64" => "REAL",
            "bool" => "INTEGER",
            "string" | "json" => "TEXT",
            t => panic!("Unknown type: {}", t),
        };

        let null_constraint = if field.required { " NOT NULL" } else { "" };

        columns.push(format!("{} {}{}", field.name, sql_type, null_constraint));
    }

    let create_table = format!(
        "CREATE TABLE IF NOT EXISTS {} (\n    {}\n)",
        table_name,
        columns.join(",\n    ")
    );

    format!(
        "pub const {}_DDL: &str = r#\"{}\"#;",
        table_name.to_uppercase(),
        create_table
    )
}

fn generate_insert_helper(schema: &Schema) -> String {
    let table_name = if schema.name == "spans" {
        "traces"
    } else {
        &schema.name
    };

    let field_names: Vec<&str> = schema.fields.iter().map(|f| f.name.as_str()).collect();
    let placeholders: Vec<&str> = schema.fields.iter().map(|_| "?").collect();

    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        table_name,
        field_names.join(", "),
        placeholders.join(", ")
    );

    let mut value_extractions = Vec::new();
    for field in &schema.fields {
        let extraction = match field.field_type.as_str() {
            "timestamp" | "int64" | "int32" => format!(r#"get_int(record, "{}")"#, field.name),
            "float64" => format!(r#"get_float(record, "{}")"#, field.name),
            "bool" => format!(r#"get_bool(record, "{}")"#, field.name),
            "string" | "json" => format!(r#"get_string(record, "{}")"#, field.name),
            t => panic!("Unknown type: {}", t),
        };
        value_extractions.push(extraction);
    }

    format!(
        r#"pub fn {}_insert_sql() -> &'static str {{
    "{}"
}}

pub fn {}_values(record: &serde_json::Value) -> Vec<SqlStorageValue> {{
    vec![
        {},
    ]
}}"#,
        table_name,
        sql,
        table_name,
        value_extractions.join(",\n        ")
    )
}

/// Generate Arrow RecordBatch converter from VRL schema.
/// Supports types: int64, int32, float64, bool, string, json.
fn generate_arrow_converter(schema: &Schema) -> String {
    let table_name = if schema.name == "spans" {
        "traces"
    } else {
        &schema.name
    };
    let mut decls = Vec::new();
    let mut appends = Vec::new();
    let mut finishes = Vec::new();

    for f in &schema.fields {
        let n = f.name.replace(".", "_").trim_start_matches('_').to_string();
        let (btype, cap, app) = match f.field_type.as_str() {
            "timestamp" | "int64" => (
                "Int64Builder",
                "rows.len()",
                format!(
                    r#"b_{n}.append_value(row.get("{0}").and_then(|v| v.as_i64()).unwrap_or(0));"#,
                    f.name
                ),
            ),
            "int32" => (
                "Int32Builder",
                "rows.len()",
                format!(
                    r#"b_{n}.append_value(row.get("{0}").and_then(|v| v.as_i64()).unwrap_or(0) as i32);"#,
                    f.name
                ),
            ),
            "float64" => (
                "Float64Builder",
                "rows.len()",
                format!(
                    r#"b_{n}.append_value(row.get("{0}").and_then(|v| v.as_f64()).unwrap_or(0.0));"#,
                    f.name
                ),
            ),
            "bool" => (
                "BooleanBuilder",
                "rows.len()",
                format!(
                    r#"b_{n}.append_value(row.get("{0}").and_then(|v| v.as_bool()).unwrap_or(false));"#,
                    f.name
                ),
            ),
            "string" | "json" if f.required => (
                "StringBuilder",
                "rows.len(), rows.len() * 32",
                format!(
                    r#"b_{n}.append_value(row.get("{0}").and_then(|v| v.as_str()).unwrap_or(""));"#,
                    f.name
                ),
            ),
            "string" | "json" => (
                "StringBuilder",
                "rows.len(), rows.len() * 32",
                format!(
                    r#"match row.get("{0}").and_then(|v| v.as_str()) {{ Some(s) => b_{n}.append_value(s), None => b_{n}.append_null() }}"#,
                    f.name
                ),
            ),
            t => panic!("Unknown type for Arrow: {}", t),
        };
        decls.push(format!("let mut b_{n} = {btype}::with_capacity({cap});"));
        appends.push(app);
        finishes.push(format!("Arc::new(b_{n}.finish())"));
    }

    format!(
        r#"pub fn json_to_{table_name}_batch(rows: &[serde_json::Value]) -> Result<arrow_array::RecordBatch, ArrowConvertError> {{
    let schema = Arc::new({0}_schema());
    if rows.is_empty() {{ return Ok(arrow_array::RecordBatch::new_empty(schema)); }}
    {1}
    for row in rows {{ {2} }}
    arrow_array::RecordBatch::try_new(schema, vec![{3}]).map_err(|e| ArrowConvertError(e.to_string()))
}}"#,
        schema.name,
        decls.join("\n    "),
        appends.join("\n        "),
        finishes.join(", ")
    )
}

fn compile_vrl_scripts() {
    let vrl_dir = Path::new("vrl");
    let schemas_dir = Path::new("schemas");
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("compiled_vrl.rs");

    let scripts = [
        ("OTLP_LOGS", "otlp_logs.vrl"),
        ("OTLP_TRACES", "otlp_traces.vrl"),
        ("OTLP_GAUGE", "otlp_gauge.vrl"),
        ("OTLP_SUM", "otlp_sum.vrl"),
        ("HEC_LOGS", "hec_logs.vrl"),
    ];

    let mut code = String::new();
    code.push_str("// Auto-generated VRL script sources\n\n");

    // Track schemas already written to avoid duplicate warnings
    let mut written_schemas: HashSet<String> = HashSet::new();

    // Collect all schemas for Arrow generation
    let mut all_schemas: Vec<Schema> = Vec::new();

    for (name, file) in scripts {
        let path = vrl_dir.join(file);
        if !path.exists() {
            panic!("VRL script not found: {}", path.display());
        }

        let source =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", file, e));

        // Parse and generate schema if annotations present
        if let Some(schema) = parse_schema_from_vrl(&source) {
            // Skip if we've already processed this schema (multiple VRL files can share a schema)
            if !written_schemas.contains(&schema.name) {
                let schema_json = generate_cloudflare_schema(&schema);
                let schema_path = schemas_dir.join(format!("{}.schema.json", schema.name));

                // Only write if content changed to avoid unnecessary warnings
                let should_write = match fs::read_to_string(&schema_path) {
                    Ok(existing) => existing != schema_json,
                    Err(_) => true, // File doesn't exist, write it
                };

                if should_write {
                    fs::write(&schema_path, &schema_json).unwrap_or_else(|e| {
                        panic!("Failed to write schema {}: {}", schema_path.display(), e)
                    });
                    println!("cargo:warning=Generated schema: {}", schema_path.display());
                }

                // Collect schema for Arrow generation
                all_schemas.push(schema);
                written_schemas.insert(all_schemas.last().unwrap().name.clone());
            }
        }

        // Validate VRL parses (we can't compile without custom functions)
        // Just check it's valid VRL syntax for now
        let _ =
            vrl::parser::parse(&source).map_err(|e| panic!("VRL parse error in {}: {:?}", file, e));

        // Embed source as const
        code.push_str(&format!(
            "pub const {}_SOURCE: &str = r#####\"{}\"#####;\n\n",
            name, source
        ));
    }

    fs::write(&dest, code).unwrap();

    // Generate Arrow schema file
    let arrow_schemas_path = Path::new(&out_dir).join("arrow_schemas.rs");
    let mut arrow_code = String::from(
        "// Auto-generated Arrow schemas from VRL\n\
         use arrow_schema::{DataType, Field, Schema};\n\
         use std::collections::HashMap;\n\n",
    );

    for schema in &all_schemas {
        arrow_code.push_str(&generate_arrow_schema(schema));
        arrow_code.push_str("\n\n");
    }

    fs::write(&arrow_schemas_path, &arrow_code).unwrap();

    // Generate SQLite DDL file
    let sqlite_ddl_path = Path::new(&out_dir).join("sqlite_ddl.rs");
    let mut ddl_code = String::from("// Auto-generated SQLite DDL from VRL\n\n");

    for schema in &all_schemas {
        ddl_code.push_str(&generate_sqlite_ddl(schema));
        ddl_code.push_str("\n\n");
    }

    fs::write(&sqlite_ddl_path, &ddl_code).unwrap();

    // Generate Insert Helpers
    let insert_helpers_path = Path::new(&out_dir).join("insert_helpers.rs");
    let mut helpers_code = String::from(
        r#"// Auto-generated insert helpers from VRL
use worker::SqlStorageValue;

fn get_int(record: &serde_json::Value, field: &str) -> SqlStorageValue {
    record.get(field)
        .and_then(|v| v.as_i64())
        .map(SqlStorageValue::Integer)
        .unwrap_or(SqlStorageValue::Null)
}

fn get_float(record: &serde_json::Value, field: &str) -> SqlStorageValue {
    record.get(field)
        .and_then(|v| v.as_f64())
        .map(SqlStorageValue::Float)
        .unwrap_or(SqlStorageValue::Null)
}

fn get_bool(record: &serde_json::Value, field: &str) -> SqlStorageValue {
    record.get(field)
        .and_then(|v| v.as_bool())
        .map(SqlStorageValue::Boolean)
        .unwrap_or(SqlStorageValue::Null)
}

fn get_string(record: &serde_json::Value, field: &str) -> SqlStorageValue {
    record.get(field)
        .and_then(|v| v.as_str())
        .map(|s| SqlStorageValue::String(s.to_string()))
        .unwrap_or(SqlStorageValue::Null)
}

"#,
    );

    for schema in &all_schemas {
        helpers_code.push_str(&generate_insert_helper(schema));
        helpers_code.push_str("\n\n");
    }

    fs::write(&insert_helpers_path, &helpers_code).unwrap();

    // Generate Arrow converters
    let arrow_convert_path = Path::new(&out_dir).join("arrow_convert_gen.rs");
    let mut convert_code = String::from(
        "// Auto-generated Arrow converters from VRL\n\
         use arrow_array::builder::{BooleanBuilder, Float64Builder, Int32Builder, Int64Builder, StringBuilder};\n\
         use std::sync::Arc;\n\n",
    );

    for schema in &all_schemas {
        // Generate converters for logs, spans (traces), gauge, and sum
        if schema.name == "logs"
            || schema.name == "spans"
            || schema.name == "gauge"
            || schema.name == "sum"
        {
            convert_code.push_str(&generate_arrow_converter(schema));
            convert_code.push_str("\n\n");
        }
    }

    fs::write(&arrow_convert_path, &convert_code).unwrap();

    println!("cargo:rerun-if-changed=vrl/");
    println!("cargo:rerun-if-changed=build.rs");
}
