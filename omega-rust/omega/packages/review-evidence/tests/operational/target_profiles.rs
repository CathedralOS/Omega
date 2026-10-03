use crate::support::*;
use compiler::CheckedCompileRequest;

#[test]
fn review_rejects_target_free_and_standalone_checked_programs() {
    let package = TempPackage::new();
    package.write("main.omg", "machine local() { }\n");

    let target_free = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), None)
    })
    .expect("target-free package fixture should check");
    let diagnostics = project_checked_package_review(&target_free)
        .expect_err("review must require an explicit target");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires one explicit target selection")
    }));

    let standalone = compile_review_fixture(CheckedCompileRequest::new(
        &package.0.join("main.omg"),
        None,
    ))
    .expect("standalone fixture should check");
    let diagnostics = project_checked_package_review(&standalone)
        .expect_err("review must require package-aware compilation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires package-aware checked compilation")
    }));
}

#[test]
fn review_encoding_ignores_unreviewed_arena_insertion_order() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write("main.omg", "boundary machine host_ping();\n");
    second.write(
        "main.omg",
        "machine unrelated() { }\nboundary machine host_ping();\n",
    );
    let build = r#"machine build(builder: &mut Build) { Build::package(builder, "review_fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
        })
        .expect("arena-order fixture should check")
    };
    let first = project_checked_package_review(&compile(&first))
        .expect("first arena-order review")
        .canonical_review_bytes()
        .expect("first arena-order encoding");
    let second = project_checked_package_review(&compile(&second))
        .expect("second arena-order review")
        .canonical_review_bytes()
        .expect("second arena-order encoding");

    assert_eq!(first, second);
}
