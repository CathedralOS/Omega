use super::fixture::{Fixture, assert_status};
use std::fs;

#[test]
fn update_explains_unsupported_generated_declaration_and_preserves_accepted_files() {
    rejected_candidate(
        "update",
        "pub machine value() -> u64 { 7 }\n",
        Some(
            r#"machine build(builder: &mut Build) {
    Build::package(builder, "arithmetic_kernels");
    let generated: BuildPath = builder.output.resolve("unsupported.generated.omg");
    let descriptor: i32 = builder.output.create(generated, 438);
    let written: i64 = builder.output.write(descriptor, "trait GeneratedOperation<T> { machine apply(value: T) -> T; }\n");
    let closed: i32 = builder.output.close(descriptor);
    builder.output.include_source(generated);
}
"#,
        ),
        &[
            "generated source uses a declaration shape not yet supported by retained-checkpoint continuation",
        ],
    );
}

#[test]
fn install_preserves_available_compiler_source_location() {
    rejected_candidate(
        "install",
        "pub machine value() -> u64 {\n    let value: u64 = ;\n    value\n}\n",
        None,
        &["expected", "main.omg:2:"],
    );
}

fn rejected_candidate(command: &str, source: &str, build: Option<&str>, reasons: &[&str]) {
    let fixture = Fixture::new();
    if command == "update" {
        assert_status(
            &fixture.omega(&["install", "../dependency", "--target", "linux_x86_64"]),
            0,
        );
        fixture.lock();
    }
    let accepted = fixture.accepted_files();
    let reviews = review_files(&fixture);
    if command == "install" {
        assert!(accepted.1.is_none());
        assert!(reviews.is_empty());
    }
    fixture.write("dependency/main.omg", source);
    if let Some(build) = build {
        fixture.write("dependency/build.omg", build);
    }
    let selection = if command == "install" {
        "../dependency"
    } else {
        "arithmetic_kernels"
    };
    let output = fixture.omega(&[command, selection, "--target", "linux_x86_64"]);
    assert_status(&output, 1);
    assert_eq!(fixture.accepted_files(), accepted);
    assert_eq!(
        review_files(&fixture),
        reviews,
        "compiler error changed review files"
    );
    for name in ["proposal", "pending"] {
        assert!(
            !fixture
                .path(&format!("root/build/package-manager/{name}"))
                .exists(),
            "compiler error created {name}"
        );
    }
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let header = "checked compilation failed for package `arithmetic_kernels`";
    let (_, diagnostics) = stderr
        .split_once(header)
        .unwrap_or_else(|| panic!("{stderr}"));
    let (_, message) = diagnostics
        .split_once("\n  error: ")
        .unwrap_or_else(|| panic!("{stderr}"));
    for reason in reasons {
        assert!(message.contains(reason), "missing {reason:?}: {stderr}");
    }
}

fn review_files(fixture: &Fixture) -> Vec<(std::ffi::OsString, Vec<u8>)> {
    let directory = fixture.path("root/build/package-manager");
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => panic!("cannot read {}: {error}", directory.display()),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("review-") || name == "source-diff.txt" {
            files.push((name, fs::read(entry.path()).unwrap()));
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}
