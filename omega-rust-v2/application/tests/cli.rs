use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn invoke(
    directory: &Path,
    arguments: &[&str],
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(directory)
        .args(arguments)
        .output()
        .expect("the omega binary must start")
}

fn application_directory() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn accepts_a_whole_file_with_v2_reference_syntax() {
    let output = invoke(
        application_directory(),
        &[
            "source",
            "parse",
            "../layers/01_tokens-to-syntax-trees/fixtures/accept/types.omg",
        ],
    );
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
}

#[test]
fn resolves_only_the_named_package_files_and_locates_missing_callees() {
    let root = application_directory().join("../layers/02_syntax-trees-to-symbol-resolved-trees/fixtures");
    let pass = invoke(
        &root.join("accept/sibling_machine"),
        &["source", "resolve", "combat/main.omg", "combat/scoring.omg"],
    );
    assert!(pass.status.success(), "{}", String::from_utf8_lossy(&pass.stderr));
    let fail = invoke(
        &root.join("reject/child_machine"),
        &["source", "resolve", "combat/main.omg", "combat/ai/scoring.omg"],
    );
    assert_eq!(fail.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&fail.stderr).trim(),
        "combat/main.omg:2:5: resolver: unresolved name score"
    );
}

#[test]
fn types_overloaded_calls_and_reports_the_wrong_result_at_its_use() {
    let root = application_directory().join("../layers/03_symbol-resolved-trees-to-typed-trees/fixtures");
    let pass = invoke(&root, &["source", "type", "draw.omg", "render.omg"]);
    assert!(pass.status.success() && pass.stderr.is_empty() && pass.stdout.is_empty());
    let fail = invoke(&root, &["source", "type", "draw.omg", "wrong_result.omg"]);
    assert_eq!(fail.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&fail.stderr).trim(),
        "wrong_result.omg:1:37: typer: expected u32, found bool"
    );
    let missing = invoke(&root, &["source", "type", "render.omg"]);
    assert!(String::from_utf8_lossy(&missing.stderr).contains("resolver: unresolved name draw"));
}

#[test]
fn types_a_computed_width_and_reports_the_overflowing_specialization() {
    let fixtures = application_directory().join("../layers/03_symbol-resolved-trees-to-typed-trees/fixtures");
    let ok = invoke(&fixtures.join("accept"), &["source", "type", "computed_width.omg"]);
    let error = invoke(&fixtures.join("reject"), &["source", "type", "computed_width.omg"]);
    assert!(ok.status.success() && ok.stdout.is_empty() && ok.stderr.is_empty());
    assert_eq!(error.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&error.stderr).trim(),
        "computed_width.omg:4:22: typer: static index 256 exceeds u8"
    );
}

#[test]
fn reports_layer_refusals_at_the_original_file_position() {
    for (path, diagnostic) in [
        (
            "../layers/01_tokens-to-syntax-trees/fixtures/reject/ampersand_reference_type.omg",
            ":1:18: parser: expected a type (bytes 17..18)",
        ),
        (
            "../layers/00_bytes-to-tokens/fixtures/reject/invalid_hex_escape.omg",
            ":1:10: lexer: \\x must be followed by exactly two hexadecimal digits (bytes 9..11)",
        ),
    ] {
        let output = invoke(application_directory(), &["source", "parse", path]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(String::from_utf8_lossy(&output.stderr).trim_end(), format!("{path}{diagnostic}"));
    }
}

#[test]
fn reads_bytes_from_an_explicit_path_in_the_callers_directory() {
    let directory = std::env::temp_dir().join(format!("omega-v2-cli-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let filename = "-two lines.omg";
    let path = directory.join(filename);
    fs::write(&path, b"// first\r\n\xff").unwrap();
    let output = invoke(&directory, &["source", "parse", "--", filename]);
    fs::remove_file(path).unwrap();
    fs::remove_dir(directory).unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim_end(),
        "-two lines.omg:2:1: lexer: source is not valid UTF-8 (bytes 10..11)"
    );
}

#[test]
fn unreadable_file_is_a_source_failure() {
    let output = invoke(application_directory(), &["source", "parse", "nonexistent input.omg"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("nonexistent input.omg: could not read source:"));
}

#[test]
fn help_and_usage_errors_have_distinct_statuses_and_streams() {
    for arguments in [
        &["--help"][..],
        &["source", "-h"],
        &["source", "parse", "--help"],
    ] {
        let output = invoke(application_directory(), arguments);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert!(String::from_utf8_lossy(&output.stdout).contains("omega source parse"));
    }
    for arguments in [
        &[][..],
        &["source"],
        &["source", "parse"],
        &["source", "parse", "--"],
        &["source", "parse", "a", "b"],
        &["source", "parse", "--typo"],
        &["source", "parze", "a"],
        &["run", "main.omg"],
    ] {
        let output = invoke(application_directory(), arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Usage: omega source parse"));
    }
}
#[test]
fn source_type_checks_inherited_rows_and_locates_ambiguous_evidence() {
    let root = application_directory().join("../layers/03_symbol-resolved-trees-to-typed-trees/fixtures");
    let accepted = invoke(&root.join("accept"), &["source", "type", "inherited_row.omg"]);
    assert!(accepted.status.success(), "{}", String::from_utf8_lossy(&accepted.stderr));
    assert!(accepted.stdout.is_empty() && accepted.stderr.is_empty());
    let refused = invoke(&root.join("reject"), &["source", "type", "evidence_ambiguity.omg"]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(refused.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&refused.stderr).trim_end(),
        "evidence_ambiguity.omg:3:5: typer: ambiguous operator +"
    );
}
