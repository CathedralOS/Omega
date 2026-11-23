//! Review retains a selected collection length's count type when checking a public clause.
use std::fs;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn reviewing_a_length_comparison_preserves_the_selected_operand_type() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "omega-review-length-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    for (label, limit) in [("count", "8"), ("boolean", "false")] {
        let project = root.join(label);
        fs::create_dir(&project).unwrap();
        fs::write(
            project.join("build.omg"),
            r#"machine build(builder: @mutable Build) { Build::package(builder, "length_contract"); }"#,
        ).unwrap();
        fs::write(
            project.join("api.omg"),
            format!(
                "public machine inspect(bytes: [u8; 8]) requires bytes.length == {limit} {{ }}"
            ),
        )
        .unwrap();
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
    fs::remove_dir_all(root).unwrap();
}
