// SPDX-License-Identifier: MIT OR Apache-2.0

// Native only, and not portable in principle: each case builds a throwaway crate
// in a temp directory and shells out to `cargo check` to assert on the
// *diagnostic* logwise_runtime_proc emits. There is no filesystem and no subprocess in
// the browser. What it covers is the proc-macro's compile-time behaviour, which
// is target-independent, so a native-only run loses nothing.
#![cfg(not(target_arch = "wasm32"))]

use std::fs;
use std::process::Command;

/// Builds a throwaway crate around `source` and runs `cargo check` on it.
///
/// Returns the (success, stderr) of the check.
fn check_fixture(name: &str, source: &str) -> (bool, String) {
    let fixture_dir = std::env::temp_dir().join(format!("logwise-{name}-{}", std::process::id()));
    let source_dir = fixture_dir.join("src");
    let _ = fs::remove_dir_all(&fixture_dir);
    fs::create_dir_all(&source_dir).expect("create macro regression fixture");

    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::write(
        fixture_dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"logwise_{name}_fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[dependencies]\nlogwise_runtime = {{ path = {:?} }}\n",
            manifest_dir
        ),
    )
    .expect("write fixture manifest");
    fs::write(source_dir.join("lib.rs"), source).expect("write fixture source");

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
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn test_logging_macro_accepts_cast_expressions() {
    let (success, stderr) = check_fixture(
        "macro-expression",
        r#"
logwise_runtime::declare_logging_domain!();

pub fn log_cast(value: u16) {
    logwise_runtime::warn_sync!("narrowed value: {value}", value = value as u8);
}
"#,
    );

    assert!(
        success,
        "a valid cast expression was mangled by the logging macro:\n{stderr}"
    );
}

#[test]
fn test_logging_macro_rejects_an_argument_with_no_key() {
    // `count` reads like `format!`'s implicit capture, but logwise has no such
    // thing. It used to be discarded silently, taking every argument after it
    // along with it.
    let (success, stderr) = check_fixture(
        "macro-bare-argument",
        r#"
logwise_runtime::declare_logging_domain!();

pub fn log_bare(count: u8) {
    logwise_runtime::warn_sync!("finished", count);
}
"#,
    );

    assert!(
        !success,
        "an argument with no `key =` was accepted and silently discarded"
    );
    assert!(
        stderr.contains("expected `key = value`"),
        "the diagnostic did not explain the missing key:\n{stderr}"
    );
}

#[test]
fn test_logging_macro_keeps_arguments_after_a_malformed_one() {
    // The malformed argument must be reported rather than truncating the list at
    // the first thing that does not parse as `key = value`.
    let (success, stderr) = check_fixture(
        "macro-trailing-argument",
        r#"
logwise_runtime::declare_logging_domain!();

pub fn log_trailing(status: u16, count: u8) {
    logwise_runtime::warn_sync!("finished {status}", status = status, count);
}
"#,
    );

    assert!(
        !success,
        "a trailing argument with no `key =` was silently discarded"
    );
    assert!(
        stderr.contains("expected `key = value`"),
        "the diagnostic did not explain the missing key:\n{stderr}"
    );
}
