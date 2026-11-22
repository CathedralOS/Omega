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
            "../layers/01_tokens-to-syntax-trees/tests/fixtures/types/accept/types.omg",
        ],
    );
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
}

#[test]
fn resolves_only_the_named_package_files_and_locates_missing_callees() {
    let root = application_directory().join("../layers/02_syntax-trees-to-symbol-resolved-trees/tests/fixtures/directory_lookup");
    let pass = invoke(
        &root.join("sibling_machine"),
        &[
            "source",
            "resolve",
            "--",
            "./combat/main.omg",
            "combat/scoring.omg",
        ],
    );
    assert!(pass.status.success(), "{}", String::from_utf8_lossy(&pass.stderr));
    let outside_package = invoke(
        &root.join("sibling_machine"),
        &["source", "resolve", "../sibling_machine/combat/main.omg"],
    );
    assert_eq!(outside_package.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&outside_package.stderr).contains("expected a path within the current package"));
    let fail = invoke(
        &root.join("child_machine"),
        &["source", "resolve", "combat/main.omg", "combat/ai/scoring.omg"],
    );
    assert_eq!(fail.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&fail.stderr).trim(),
        "combat/main.omg:2:5: resolver: unresolved name score (bytes 28..33)"
    );
}

#[test]
fn types_overloaded_calls_and_reports_the_wrong_result_at_its_use() {
    let root = application_directory().join("../layers/03_symbol-resolved-trees-to-typed-trees/tests/fixtures/overloaded_calls");
    let pass = invoke(&root, &["source", "type", "draw.omg", "render.omg"]);
    assert!(pass.status.success() && pass.stderr.is_empty() && pass.stdout.is_empty());
    let fail = invoke(&root, &["source", "type", "draw.omg", "wrong_result.omg"]);
    assert_eq!(fail.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&fail.stderr).trim(),
        "wrong_result.omg:1:37: typer: expected u32, found bool (bytes 36..46)"
    );
    let missing = invoke(&root, &["source", "type", "render.omg"]);
    assert!(String::from_utf8_lossy(&missing.stderr).contains("resolver: unresolved name draw"));
}

#[test]
fn prepares_a_signed_index_and_reports_exact_intermediate_overflow() {
    let root = application_directory().join("../layers/04_typed-trees-to-prepared-trees/tests/fixtures/signed_arithmetic");
    let pass = invoke(&root, &["source", "prepare", "signed_wrap.omg"]);
    assert!(pass.status.success(), "{}", String::from_utf8_lossy(&pass.stderr));
    assert!(pass.stdout.is_empty() && pass.stderr.is_empty());
    let fail = invoke(&root, &["source", "prepare", "signed_overflow.omg"]);
    assert_eq!(fail.status.code(), Some(1));
    let diagnostic = String::from_utf8_lossy(&fail.stderr);
    assert!(diagnostic.contains("signed_overflow.omg:3:19:"), "{diagnostic}");
    assert!(diagnostic.contains("exact addition exceeds its selected integer carrier"), "{diagnostic}");
}

#[test]
fn types_a_computed_width_and_reports_the_overflowing_specialization() {
    let fixtures = application_directory().join("../layers/03_symbol-resolved-trees-to-typed-trees/tests/fixtures/computed_width");
    let ok = invoke(&fixtures.join("accept"), &["source", "type", "computed_width.omg"]);
    let error = invoke(&fixtures.join("reject"), &["source", "type", "computed_width.omg"]);
    assert!(ok.status.success() && ok.stdout.is_empty() && ok.stderr.is_empty());
    for directory in ["prepared/accept", "prepared/reject"] {
        let missing = invoke(&fixtures.join(directory), &["source", "type", "computed_width.omg"]);
        assert_eq!(missing.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&missing.stderr).contains("needs an admitted evaluation result"));
    }
    let prepared = invoke(&fixtures.join("prepared/accept"), &["source", "prepare", "computed_width.omg"]);
    assert!(prepared.status.success(), "{}", String::from_utf8_lossy(&prepared.stderr));
    assert!(prepared.stdout.is_empty() && prepared.stderr.is_empty());
    let refused = invoke(&fixtures.join("prepared/reject"), &["source", "prepare", "computed_width.omg"]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("exact addition exceeds its selected integer carrier"));
    assert_eq!(error.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&error.stderr).trim(),
        "computed_width.omg:3:22: typer: static index 256 exceeds u8 (bytes 116..129)"
    );
}

#[test]
fn reports_layer_refusals_at_the_original_file_position() {
    for (path, diagnostic) in [
        (
            "../layers/01_tokens-to-syntax-trees/tests/fixtures/types/reject/ampersand_reference_type.omg",
            ":1:18: parser: expected a type (bytes 17..18)",
        ),
        (
            "../layers/00_bytes-to-tokens/tests/fixtures/quoted_literals/reject/invalid_hex_escape.omg",
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
        &["source", "resolve", "--help"],
        &["source", "type", "-h"],
        &["source", "prepare", "--help"],
        &["source", "lower", "--help"],
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
        &["source", "resolve", "--unknown"],
        &["source", "type", "--"],
        &["source", "resolve"],
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
    let root = application_directory().join("../layers/03_symbol-resolved-trees-to-typed-trees/tests/fixtures/conformances");
    let accepted = invoke(&root.join("accept"), &["source", "type", "inherited_row.omg"]);
    assert!(accepted.status.success(), "{}", String::from_utf8_lossy(&accepted.stderr));
    assert!(accepted.stdout.is_empty() && accepted.stderr.is_empty());
    let refused = invoke(&root.join("reject"), &["source", "type", "evidence_ambiguity.omg"]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(refused.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&refused.stderr).trim_end(),
        "evidence_ambiguity.omg:3:5: typer: ambiguous operator + (bytes 130..135)"
    );
}

#[test]
fn help_reports_an_unwritable_output_stream() {
    let read_only = fs::File::open(application_directory().join("Cargo.toml")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(application_directory())
        .arg("--help")
        .stdout(read_only)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("omega: could not write help:"));
}

#[cfg(any(unix, windows))]
#[test]
fn resolve_refuses_a_filename_that_cannot_identify_a_source_module() {
    #[cfg(windows)]
    let path = <std::ffi::OsString as std::os::windows::ffi::OsStringExt>::from_wide(&[0xd800]);
    #[cfg(unix)]
    let path = <std::ffi::OsString as std::os::unix::ffi::OsStringExt>::from_vec(vec![0xff]);
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(application_directory())
        .args(["source", "resolve", "--"])
        .arg(path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("logical paths must be UTF-8"));
}

// Keep the original two-file CLI packages, including their control inputs. The
// formation_handoff and predicate_call_handoff layer tests inspect the actual Psi.
fn assert_packages_prepare_and_lower(
    label: &str,
    sources: [&str; 2],
) {
    let directory = application_directory()
        .join("../target/frontend-diagnostic-cases")
        .join(format!("{}-{label}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("other.omg"), b"machine other() {}").unwrap();
    for source in sources {
        fs::write(directory.join("case.omg"), source).unwrap();
        for action in ["prepare", "lower"] {
            let output = invoke(&directory, &["source", action, "other.omg", "case.omg"]);
            assert!(
                output.status.success(),
                "source {action} on {source}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stdout.is_empty() && output.stderr.is_empty());
        }
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn checking_a_replacement_uses_the_input_value_before_the_cast() {
    let directory = application_directory()
        .join("../target")
        .join(format!("checked-replacement-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let declarations = "domain i32::Km; domain i32::Degrees; domain i32::Positive requires self > 0;";
    for (value, added, accepts) in [
        ("-7", "Degrees", true),
        ("7", "Positive", true),
        ("-7", "Positive", false),
    ] {
        let cast = format!("x as -Km +{added}");
        let source = format!("{declarations}\nmachine relabel()->i32 in {added} {{ let x = ({value}i32) as +Km; {cast} }}");
        fs::write(directory.join("cast.omg"), &source).unwrap();
        let lowered = invoke(&directory, &["source", "lower", "cast.omg"]);
        assert!(lowered.status.success(), "{}", String::from_utf8_lossy(&lowered.stderr));
        let checked = invoke(
            &directory,
            &[
                "source",
                "check",
                "--only",
                "scalar-replacements",
                "--package-root",
                ".",
                "cast.omg",
            ],
        );
        let diagnostic = String::from_utf8(checked.stderr).unwrap();
        assert_eq!(checked.status.code(), Some(if accepts { 0 } else { 1 }), "{source}: {diagnostic}");
        if accepts {
            assert!(diagnostic.is_empty());
        } else {
            let at = source.find(&cast).unwrap();
            assert!(
                diagnostic.contains("checker:") && diagnostic.contains("Positive") && diagnostic.contains(&format!("(bytes {at}..{})", at + cast.len())),
                "{diagnostic}"
            );
        }
    }
    let incomplete = invoke(&directory, &["source", "check", "cast.omg"]);
    assert_eq!(incomplete.status.code(), Some(2));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn lowers_omitted_and_written_zero_initializers() {
    let omitted = "data Options {count:u8; retries:u16;} machine build(){let options=Options{count:1,..};}";
    let written = omitted.replace("count:1,..", "count:1,retries:0");
    assert_packages_prepare_and_lower("zero-field", [omitted, &written]);
}

#[test]
fn lowers_called_and_inline_domain_predicates() {
    let inline = "machine predicate()->bool reaches none crashes none terminates {true}\r\ndomain i32::Guarded requires true && self > 0;";
    let called = inline.replace("self > 0", "predicate()");
    assert_packages_prepare_and_lower("domain-predicate", [&called, inline]);
}

#[test]
fn lowers_prepared_array_extent_and_runtime_arithmetic() {
    let root = application_directory().join("../layers/05_prepared-trees-to-terminal-psi/tests/fixtures/array_extent");
    let ok = invoke(&root, &["source", "lower", "extent.omg"]);
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));
    assert!(ok.stdout.is_empty() && ok.stderr.is_empty());
    let prepared = invoke(&root, &["source", "prepare", "runtime_operation.omg"]);
    assert!(prepared.status.success(), "{}", String::from_utf8_lossy(&prepared.stderr));
    let lowered = invoke(&root, &["source", "lower", "runtime_operation.omg"]);
    assert!(lowered.status.success(), "{}", String::from_utf8_lossy(&lowered.stderr));
    assert!(lowered.stdout.is_empty() && lowered.stderr.is_empty());
}

#[test]
fn default_build_captures_only_the_explicit_root_and_keeps_focused_argv_bounded() {
    let cwd = std::env::temp_dir().join(format!("omega-cli-captured-root-{}", std::process::id()));
    fs::create_dir(&cwd).unwrap();
    let root = cwd.join("package");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("-entry.omg"), "machine entry() {}").unwrap();
    let args = ["source", "lower", "--package-root", "package", "--", "-entry.omg"];

    // This unselected sibling of the package exceeds the capture limit. The
    // explicit entrance cannot inspect it, even though it runs from this cwd.
    let large = cwd.join("unrelated.bin");
    fs::File::create(&large).unwrap().set_len(256 * 1024 * 1024 + 1).unwrap();
    let captured = invoke(&cwd, &args);
    assert!(captured.status.success(), "{}", String::from_utf8_lossy(&captured.stderr));

    // Moving those bytes inside the explicit package makes capture fail.
    // The positional compatibility entrance still reads just its named file.
    fs::rename(&large, root.join("large.bin")).unwrap();
    let over = invoke(&cwd, &args);
    assert_eq!(over.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&over.stderr).contains("package source exceeds capture byte limit"));
    let focused = invoke(&root, &["source", "lower", "--", "-entry.omg"]);
    assert!(focused.status.success(), "{}", String::from_utf8_lossy(&focused.stderr));
    fs::remove_dir_all(cwd).unwrap();
}

#[test]
fn lowering_from_an_explicit_capture_sees_the_requesters_build_owner() {
    let root = application_directory().join("../layers/04_typed-trees-to-prepared-trees/tests/fixtures/access_policy");
    let focused = invoke(&root, &["source", "lower", "selected.omg"]);
    assert_eq!(focused.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&focused.stderr).contains("domain access policy needs the package owner for its semantic schema"));
    let captured = invoke(application_directory(), &["source", "lower", "--package-root", root.to_str().unwrap(), "--", "selected.omg"]);
    assert!(captured.status.success(), "{}", String::from_utf8_lossy(&captured.stderr));
}
