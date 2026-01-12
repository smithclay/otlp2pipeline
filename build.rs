use std::{env, fs, path::Path};

fn main() {
    write_cloudflare_schemas();
}

fn write_cloudflare_schemas() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let schemas_dir = Path::new(&manifest_dir).join("schemas");
    fs::create_dir_all(&schemas_dir).expect("failed to create schemas directory");

    for schema in otlp2records::schema_defs() {
        let schema_json = generate_cloudflare_schema(schema);
        let schema_path = schemas_dir.join(format!("{}.schema.json", schema.name));

        let should_write = match fs::read_to_string(&schema_path) {
            Ok(existing) => existing != schema_json,
            Err(_) => true,
        };

        if should_write {
            fs::write(&schema_path, &schema_json).unwrap_or_else(|e| {
                panic!("failed to write schema {}: {}", schema_path.display(), e)
            });
            println!("cargo:warning=Generated schema: {}", schema_path.display());
        }
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../otlp2records/vrl/");
}

fn generate_cloudflare_schema(schema: &otlp2records::SchemaDef) -> String {
    let mut fields_json = Vec::new();

    for field in schema.fields {
        let field_obj = format!(
            r#"    {{ "name": "{}", "type": "{}", "required": {} }}"#,
            field.name, field.field_type, field.required
        );
        fields_json.push(field_obj);
    }

    format!(
        "{{\n  \"fields\": [\n{}\n  ]\n}}\n",
        fields_json.join(",\n")
    )
}
