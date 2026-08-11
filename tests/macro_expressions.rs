// SPDX-License-Identifier: MIT OR Apache-2.0

#![cfg(not(target_arch = "wasm32"))]

use std::fs;
use std::process::Command;

#[test]
fn test_logging_macro_accepts_cast_expressions() {
    let fixture_dir =
        std::env::temp_dir().join(format!("logwise-macro-expression-{}", std::process::id()));
    let source_dir = fixture_dir.join("src");
    fs::create_dir_all(&source_dir).expect("create macro regression fixture");

    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::write(
        fixture_dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"logwise_macro_expression_fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[dependencies]\nlogwise = {{ path = {:?} }}\n",
            manifest_dir
        ),
    )
    .expect("write fixture manifest");
    fs::write(
        source_dir.join("lib.rs"),
        r#"
logwise::declare_logging_domain!();

pub fn log_cast(value: u16) {
    logwise::warn_sync!("narrowed value: {value}", value = value as u8);
}
"#,
    )
    .expect("write fixture source");

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .arg("check")
        .arg("--offline")
        .arg("--manifest-path")
        .arg(fixture_dir.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(fixture_dir.join("target"))
        .output()
        .expect("run cargo check for macro regression fixture");

    let _ = fs::remove_dir_all(&fixture_dir);
    assert!(
        output.status.success(),
        "a valid cast expression was mangled by the logging macro:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
