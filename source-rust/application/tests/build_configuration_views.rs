//! A Build returns byte views through its plan; preparation validates their values.

use std::process::Command;

fn check(name: &str, assignments: &str) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-build-configuration-view-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("staging directory");
    std::fs::write(root.join("main.omg"), "machine main() { }\n").expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        format!(
            "use main.omg as app;
use {{omega}}/build/storage.omg;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: \"configured\" }};
{assignments}
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
}}
"
        ),
    )
    .expect("staged Build");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--check", "--target", "linux_x86_64"])
        .arg("build.omg")
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    let _ = std::fs::remove_dir_all(&root);
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn a_build_that_assigns_no_view_compiles() {
    let (compiled, stderr) = check("control", "    builder.tests.disabled = true;\n");
    assert!(compiled, "{stderr}");
}

#[test]
fn build_refuses_the_retired_freestanding_field() {
    let (compiled, stderr) = check("freestanding", "    builder.freestanding = true;\n");
    assert!(!compiled, "{stderr}");
    assert!(
        stderr.contains("data `BuildPlan` has no field `freestanding`"),
        "{stderr}"
    );
}

/// A view field of the Build itself, and one of a record the Build holds.
#[test]
fn a_build_that_assigns_a_literal_to_a_view_field_compiles() {
    let (compiled, stderr) = check(
        "identifier",
        "    builder.identifier = \"com.example.settled\";\n",
    );
    assert!(compiled, "{stderr}");
    let (compiled, stderr) = check(
        "nested",
        "    builder.elf_interpreter.linux_x86_64 = \"/lib64/ld-linux-x86-64.so.2\";\n",
    );
    assert!(compiled, "{stderr}");
}

/// The twin differs only in the literal's bytes. The toolchain reads the
/// identifier the Build assigned, so the literal reached the field as the
/// view it is.
#[test]
fn an_assigned_identifier_is_the_value_the_toolchain_reads() {
    let (compiled, stderr) = check(
        "bad-identifier",
        "    builder.identifier = \"not an identifier!\";\n",
    );
    assert!(!compiled);
    assert!(
        stderr.contains("Build.identifier is not a valid application identifier"),
        "{stderr}"
    );
}

/// A value that is no byte view does not become one by being assigned to a
/// view field.
#[test]
fn a_value_that_is_no_view_is_not_assigned_to_a_view_field() {
    let (compiled, stderr) = check("boolean", "    builder.identifier = true;\n");
    assert!(!compiled, "{stderr}");
}

#[test]
fn the_returned_plan_requires_invocation_backing_for_a_computed_name() {
    let source = "let mutable bytes: [u8; 3] = [0, 105, 116];
    bytes[0] = 107u8;
    builder.product = BuildProduct::Application { name: |storage|::copy(builder, @bytes[..]) };
    bytes[0] = 0u8;";
    let (compiled, stderr) = check("copied-name", source);
    assert!(compiled, "{stderr}");

    let dangling = source.replace("|storage|::copy(builder, @bytes[..])", "@bytes[..]");
    let (compiled, stderr) = check("dangling-name", &dangling);
    assert!(!compiled, "{stderr}");
    assert!(
        stderr.contains("local storage cannot establish that promise"),
        "{stderr}"
    );
}
