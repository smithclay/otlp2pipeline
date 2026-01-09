// tests/cli_init_test.rs
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Get the path to the built binary
fn get_binary_path() -> PathBuf {
    // Build the binary first
    let build_status = Command::new("cargo")
        .args(["build", "--quiet"])
        .status()
        .expect("Failed to build");
    assert!(build_status.success(), "Build failed");

    // Return the path to the debug binary
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push("otlp2pipeline");
    path
}

#[test]
fn test_init_creates_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join(".otlp2pipeline.toml");
    let binary = get_binary_path();

    let output = Command::new(&binary)
        .args(["init", "--provider", "cf", "--env", "test"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run command");

    assert!(
        output.status.success(),
        "Command failed: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(config_path.exists(), "Config file not created");

    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("provider = \"cloudflare\""));
    assert!(content.contains("environment = \"test\""));
}

#[test]
fn test_init_refuses_overwrite_without_force() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join(".otlp2pipeline.toml");
    fs::write(&config_path, "existing").unwrap();
    let binary = get_binary_path();

    let output = Command::new(&binary)
        .args(["init", "--provider", "cf", "--env", "test"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run command");

    assert!(!output.status.success(), "Should have failed");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("already exists") || stderr.contains("force"),
        "Expected error about existing file or force flag, got: {}",
        stderr
    );
}
