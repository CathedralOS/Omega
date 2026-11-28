//! Package review checks the types of collection lengths in exported declarations.
use super::project;

use std::process::{Command, Stdio};

#[test]
fn reviewing_a_length_comparison_preserves_the_selected_operand_type() {
    let root = project::stage("review-length", &[]);
    for (label, limit) in [("count", "8"), ("boolean", "false")] {
        let project = root.path().join(label);
        let api = format!(
            "public machine inspect(bytes: [u8; 8]) requires bytes.length == {limit} {{ }}"
        );
        project::write(
            &project,
            &[
                (
                    "build.omg",
                    r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "length_contract" }; }"#,
                ),
                ("api.omg", &api),
            ],
        );
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .args([
                "packages",
                "update",
                "--offline",
                "--target",
                "macos_arm64",
                "--project",
            ])
            .arg(&project)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let error = String::from_utf8_lossy(&output.stderr);
        if label == "count" {
            assert!(output.status.success(), "{error}");
            assert!(project.join("omega.lock").is_file());
        } else {
            assert!(!output.status.success());
            assert!(error.contains("clause operand types differ"), "{error}");
            assert!(!project.join("omega.lock").exists());
        }
    }
}

#[test]
fn reviewing_an_open_array_extent_checks_its_type_before_application() {
    // No machine or closed application supplies a value for Count in this package.
    let build = r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "open_extent" }; }"#;
    for (operator, valid) in [("+", true), ("<", false)] {
        let api = format!(
            "public const Bias: u64 = 1u64;
             public data Padded<const Count: u64> = [u8; Count {operator} Bias];"
        );
        let (accepted, diagnostic) = project::check(
            "review-open-extent",
            &[("build.omg", build), ("api.omg", &api)],
            &[
                "packages",
                "update",
                "--offline",
                "--target",
                "linux_x86_64",
                "--project",
                ".",
            ],
        );
        assert_eq!(accepted, valid, "{diagnostic}");
        if !valid {
            assert!(
                diagnostic.contains("a fixed array length must be an integer"),
                "{diagnostic}"
            );
        }
    }
}
