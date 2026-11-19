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
            "../layers/01-parser/fixtures/accept/types.omg",
        ],
    );
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
}

#[test]
fn reports_layer_refusals_at_the_original_file_position() {
    for (path, diagnostic) in [
        (
            "../layers/01-parser/fixtures/reject/ampersand_reference_type.omg",
            ":1:18: parser: expected a type (bytes 17..18)",
        ),
        (
            "../layers/00-lexer/fixtures/reject/invalid_hex_escape.omg",
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
