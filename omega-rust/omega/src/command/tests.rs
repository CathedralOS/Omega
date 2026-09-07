use super::*;
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
            "--output-only",
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
    assert!(parsed.output_only);
    assert_eq!(parsed.build_dir, Some(PathBuf::from("build directory")));
    assert_eq!(parsed.target_name.as_deref(), Some("linux_x64"));
    assert!(!parsed.optimization_rollback.is_empty());
    assert_eq!(parsed.root_path, PathBuf::from("main.omg"));
}

#[test]
fn compilation_rejects_duplicate_offline_and_missing_root() {
    for (arguments, expected) in [
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
    for option in ["--build-dir", "--target", "--disable-optimization"] {
        let result = parse_arguments(
            [option, "--offline", "main.omg"]
                .into_iter()
                .map(OsString::from),
        );
        assert!(matches!(result, Err(error) if error.contains("requires")));
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
