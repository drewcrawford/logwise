// SPDX-License-Identifier: MIT OR Apache-2.0

#[test]
fn test_release_script_does_not_reference_an_unrelated_application() {
    let script = include_str!("../scripts/native/release");
    assert!(
        !script.contains("Vectropolis"),
        "the logwise release script still packages the unrelated Vectropolis application"
    );
    assert!(
        script.contains("cargo build --release"),
        "the logwise release script should build this crate in release mode"
    );
}
