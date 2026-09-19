use super::{PathBuf, parse_arguments, usage};
use std::ffi::OsString;

#[test]
fn compilation_defaults_online_and_preserves_offline_in_either_order() {
    for check in [false, true] {
        for offline_position in [None, Some(0), Some(1)] {
            let mut arguments = vec!["main.omg"];
            if let Some(position) = offline_position {
                arguments.insert(position, "--offline");
            }
            if check {
                arguments.push("--check");
            }
            let parsed = parse_arguments(arguments.iter().map(OsString::from)).unwrap();
            assert_eq!(parsed.offline, offline_position.is_some());
            assert_eq!(parsed.check_only, check);
            assert!(!parsed.timings);
            assert!(parsed.build_inputs.is_none());
            assert_eq!(parsed.root_path, PathBuf::from("main.omg"));
        }
    }
}

#[test]
fn offline_combines_with_existing_compilation_options() {
    let parsed = parse_arguments(
        [
            "--offline",
            "--accept-admissions",
            "--timings",
            "--build-dir",
            "build directory",
            "--target",
            "linux_x64",
            "--disable-optimization",
            "ControlFlowCleanup",
            "main.omg",
        ]
        .into_iter()
        .map(OsString::from),
    )
    .unwrap();
    assert!(parsed.offline);
    assert!(parsed.accept_admissions);
    assert!(parsed.timings);
    assert_eq!(parsed.build_dir, Some(PathBuf::from("build directory")));
    assert_eq!(parsed.target_name.as_deref(), Some("linux_x64"));
    assert!(!parsed.optimization_rollback.is_empty());
    assert_eq!(parsed.root_path, PathBuf::from("main.omg"));
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
fn compilation_collects_explicit_required_and_optional_build_inputs() {
    use package_compilation::BuildSourceCaptureObligation::{Optional, Required};
    let parsed = parse_arguments(
        [
            "--build-input",
            "main.omg",
            "project/main.omg",
            "--optional-build-input",
            "absent.txt",
            "--build-input",
            "templates",
            "--build-input",
            "build.omg",
        ]
        .into_iter()
        .map(OsString::from),
    )
    .unwrap();
    let entries = parsed
        .build_inputs
        .as_ref()
        .unwrap()
        .entries()
        .collect::<Vec<_>>();
    assert_eq!(
        entries,
        vec![
            (b"absent.txt".as_slice(), Optional),
            (b"build.omg".as_slice(), Required),
            (b"main.omg".as_slice(), Required),
            (b"templates".as_slice(), Required),
        ]
    );
    assert_eq!(parsed.root_path, PathBuf::from("project/main.omg"));
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
fn compilation_rejects_obsolete_package_root_policy_as_an_unknown_option() {
    for arguments in [
        vec!["--package-root-policy"],
        vec!["--package-root-policy", "policy.txt", "main.omg"],
        vec!["--check", "main.omg", "--package-root-policy", "policy.txt"],
    ] {
        let result = parse_arguments(arguments.iter().map(OsString::from));
        assert!(
            matches!(result, Err(error) if error == "unrecognized option `--package-root-policy`")
        );
    }
    assert!(!usage().contains("--package-root-policy"));
}
