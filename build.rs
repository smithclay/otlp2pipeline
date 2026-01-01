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
            } else {
                println!(
                    "cargo:warning=Failed to parse schema field: {}",
                    line.trim_start_matches("# ")
                );
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

    for (name, file) in scripts {
        let path = vrl_dir.join(file);
        if !path.exists() {
            panic!("VRL script not found: {}", path.display());
        }

        let source =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {}: {}", file, e));

        // Parse and generate Cloudflare Pipeline schema if annotations present
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

                written_schemas.insert(schema.name);
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

    println!("cargo:rerun-if-changed=vrl/");
    println!("cargo:rerun-if-changed=build.rs");
}
