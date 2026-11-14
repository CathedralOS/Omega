/*

End-to-end CLI check that a Build may assign a quoted literal to a view field
of its configuration (`omega --check`).

`builder.identifier = "com.example.settled";` writes a byte view into a field
declared `&'storage [u8]`. A quoted literal "has ordinary shared byte-view
type `&[u8]`" (`wiki/spec/language/data_and_literals.md`, "Quoted bytes"), so
the literal is built at the field's own declared type. Assigning to any other
binding of reference type writes through the reference instead, and is not
this case.

We stage a project whose Build assigns one such field and run `omega
--check`. The control assigns none. The twins show the literal arrives as the
value it is: the toolchain reads the identifier the Build wrote and refuses
one with a byte an application identifier may not hold, and a value that is
no view is still refused.

*/

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
            "machine build(builder: &mut Build) {{
    Build::application(builder, \"configured\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
{assignments}}}
"
        ),
    )
    .expect("staged Build");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--check")
        .arg("main.omg")
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
    let (compiled, stderr) = check("control", "    builder.freestanding = true;\n");
    assert!(compiled, "{stderr}");
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
