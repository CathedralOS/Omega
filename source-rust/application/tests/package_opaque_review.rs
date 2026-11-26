//! Package review keeps opaque declarations apart from physical representations.
use platform_custody::temporary_directory::TemporaryDirectory;
use std::{fs, process::Command};

#[test]
fn reference_only_opaque_data_has_declarations_without_a_physical_representation() {
    let project = TemporaryDirectory::create("review-opaque-declarations", false).unwrap();
    fs::write(
        project.path().join("build.omg"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "tables"); }"#,
    )
    .unwrap();
    let declarations = "public boundary data EfiSystemTable;\nboundary data PrivateTable;\npublic data TableBytes { word: u64; }";
    let audit = |source: &str| {
        fs::write(project.path().join("tables.omg"), source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(project.path())
            .args([
                "packages",
                "audit",
                "--offline",
                "--target",
                "linux_x86_64",
                "--details",
            ])
            .output()
            .unwrap();
        (
            output.status.code(),
            String::from_utf8(output.stdout).unwrap(),
        )
    };
    let (status, report) = audit(&format!(
        "{declarations}\npublic machine present<'r>(table: @'r EfiSystemTable) -> bool {{ true }}"
    ));
    assert_eq!(status, Some(0), "{report}");
    assert!(
        report.contains("opaque-representation declarations 2 candidates 0 selections 0 demands 0"),
        "{report}"
    );
    let (declarations, _) = report
        .split_once("field representation {")
        .unwrap()
        .1
        .split_once("field producer_availability {")
        .unwrap();
    assert!(declarations.contains(r#"string "tables.omg::EfiSystemTable""#));
    assert!(declarations.contains(r#"string "tables.omg::PrivateTable""#));
    assert!(!declarations.contains("tables.omg::TableBytes"));
    let public_api = report
        .split_once("field public_api {")
        .unwrap()
        .1
        .split_once("field callables {")
        .unwrap()
        .0;
    assert!(public_api.contains(r#"string "tables.omg::EfiSystemTable""#));
    assert!(public_api.contains("tag boundary_opaque 1"));
    assert!(!public_api.contains("tables.omg::PrivateTable"));

    let (status, report) = audit(
        r#"use {omega}/language/core/representation.omg;
public boundary data EfiSystemTable;
public data TableBytes { word: u64; }
public TableRepresentation satisfies |representation|::OpaqueRepresentation<EfiSystemTable, TableBytes> { }"#,
    );
    assert_eq!(status, Some(1), "{report}");
    assert!(
        report.contains("opaque representation producer availability"),
        "{report}"
    );
}
