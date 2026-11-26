//! Review retains a selected collection length's count type when checking a public clause.
#[path = "support/project.rs"]
mod project;

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
                    r#"machine build(builder: @mutable Build) { Build::package(builder, "length_contract"); }"#,
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
