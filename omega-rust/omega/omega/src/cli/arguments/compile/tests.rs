use super::parse_arguments;
use std::ffi::OsString;

#[test]
fn compilation_rejects_duplicate_and_missing_report_file() {
    for (arguments, expected) in [
        (
            vec![
                "--report-file",
                "a.txt",
                "--report-file",
                "b.txt",
                "main.omg",
            ],
            "duplicate --report-file",
        ),
        (vec!["main.omg", "--report-file"], "--report-file requires"),
        (
            vec!["--report-file", "--check", "main.omg"],
            "--report-file requires",
        ),
    ] {
        let result = parse_arguments(arguments.iter().map(OsString::from));
        assert!(matches!(result, Err(error) if error.contains(expected)));
    }
}

#[test]
fn compilation_rejects_duplicate_offline_and_missing_root() {
    for (arguments, expected) in [
        (
            vec!["--timings", "main.omg", "--timings"],
            "duplicate --timings",
        ),
        (
            vec!["--offline", "main.omg", "--offline"],
            "duplicate --offline",
        ),
        (
            vec!["--check", "--offline"],
            "missing root Omega source path",
        ),
        (vec!["--offline=true", "main.omg"], "unrecognized option"),
    ] {
        let result = parse_arguments(arguments.iter().map(OsString::from));
        assert!(matches!(result, Err(error) if error.contains(expected)));
    }
}

#[test]
fn compilation_never_consumes_offline_as_an_option_value() {
    for option in [
        "--build-dir",
        "--target",
        "--disable-optimization",
        "--build-input",
        "--optional-build-input",
    ] {
        let result = parse_arguments(
            [option, "--offline", "main.omg"]
                .into_iter()
                .map(OsString::from),
        );
        assert!(matches!(result, Err(error) if error.contains("requires")));
    }
}

#[test]
fn compilation_rejects_invalid_and_overlapping_build_input_paths() {
    for path in [
        "",
        ".",
        "..",
        "../secret",
        "/absolute",
        "C:/drive",
        "a\\b",
        "a//b",
        "a/./b",
        "a\0b",
    ] {
        let result = parse_arguments(
            ["main.omg", "--build-input", path]
                .into_iter()
                .map(OsString::from),
        );
        assert!(
            matches!(result, Err(error) if error.contains("canonical relative path")),
            "{path:?}"
        );
    }
    for (first, second, expected) in [
        ("a", "a", "declared twice"),
        ("a", "a/b", "nests inside"),
        ("a/b", "a", "nests inside"),
    ] {
        let result = parse_arguments(
            [
                "main.omg",
                "--build-input",
                first,
                "--optional-build-input",
                second,
            ]
            .into_iter()
            .map(OsString::from),
        );
        assert!(matches!(result, Err(error) if error.contains(expected)));
    }
    for option in ["--build-input", "--optional-build-input"] {
        let result = parse_arguments(["main.omg", option].into_iter().map(OsString::from));
        assert!(matches!(result, Err(error) if error.contains("requires")));
    }
}

#[cfg(unix)]
#[test]
fn compilation_rejects_non_utf8_build_inputs() {
    use std::os::unix::ffi::OsStringExt;
    for option in ["--build-input", "--optional-build-input"] {
        let result = parse_arguments(
            [
                OsString::from("main.omg"),
                OsString::from(option),
                OsString::from_vec(vec![0xff]),
            ]
            .into_iter(),
        );
        assert!(matches!(result, Err(error) if error.contains("UTF-8")));
    }
}

#[test]
fn repeated_targets_form_a_set_in_the_order_named() {
    let parsed = parse_arguments(
        [
            "--target",
            "windows_x86_64",
            "--target",
            "linux_arm64",
            "--target",
            "windows_x86_64",
            "main.omg",
        ]
        .into_iter()
        .map(OsString::from),
    )
    .unwrap();
    assert_eq!(parsed.target_names, ["windows_x86_64", "linux_arm64"]);

    let parsed = parse_arguments(["main.omg"].into_iter().map(OsString::from)).unwrap();
    assert!(parsed.target_names.is_empty());
}
