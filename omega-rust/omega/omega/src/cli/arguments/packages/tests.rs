use super::{OsString, PackageCommandKind, parse_arguments};
mod named;
mod offline;

#[test]
fn package_input_paths_reject_invalid_or_ambiguous_inventories() {
    for arguments in [
        vec!["--build-input", "../secret"],
        vec!["--build-input", "/absolute"],
        vec!["--build-input", "C:/drive"],
        vec!["--build-input", "a", "--build-input", "a/child"],
        vec!["--build-input", "a", "--optional-build-input", "a"],
    ] {
        assert!(
            parse_arguments(
                PackageCommandKind::Update,
                arguments.iter().map(OsString::from)
            )
            .is_err(),
            "{arguments:?}"
        );
    }
}

fn rejects(kind: PackageCommandKind, arguments: &[&str], expected: &str) {
    match parse_arguments(kind, arguments.iter().map(OsString::from)) {
        Err(error) => assert!(error.contains(expected), "{arguments:?}: {error}"),
        Ok(_) => panic!("accepted invalid arguments {arguments:?}"),
    }
}

#[test]
fn duplicate_singleton_options_and_package_selections_reject() {
    for arguments in [
        vec!["source", "--rev", "v1", "--rev", "v1"],
        vec!["source", "--as", "alias", "--as", "alias"],
        vec!["source", "--project", ".", "--project", "."],
        vec!["--resume", "--resume"],
        vec!["--discard-review", "--discard-review"],
        vec!["--help", "--help"],
    ] {
        rejects(PackageCommandKind::Install, &arguments, "duplicate");
    }
    rejects(
        PackageCommandKind::Update,
        &["--to", "v1", "--to", "v1"],
        "duplicate",
    );
    rejects(
        PackageCommandKind::Update,
        &["package", "package"],
        "duplicate package",
    );
}

#[test]
fn duplicate_targets_reject_on_the_same_canonical_profile() {
    rejects(
        PackageCommandKind::Update,
        &["--target", "linux_x86_64", "--target", "linux_x86_64"],
        "duplicate target",
    );
    // A retired alias never reaches the duplicate check: it is an unknown
    // target profile, not a second spelling of one.
    rejects(
        PackageCommandKind::Update,
        &["--target", "linux_x64", "--target", "linux_x86_64"],
        "unknown target",
    );
}

#[test]
fn unknown_options_targets_and_wrong_command_options_reject() {
    for option in ["--bogus", "-x", "--", "--rev=v1", "--project=."] {
        rejects(
            PackageCommandKind::Install,
            &[option],
            "unrecognized option",
        );
    }
    rejects(
        PackageCommandKind::Update,
        &["--target", "all"],
        "unknown target",
    );
    rejects(
        PackageCommandKind::Install,
        &["source", "--to", "v1"],
        "not valid",
    );
    rejects(PackageCommandKind::Update, &["--rev", "v1"], "not valid");
    rejects(
        PackageCommandKind::Update,
        &["--as", "alias"],
        "only valid for install",
    );
    rejects(PackageCommandKind::Install, &[], "requires a source");
    rejects(
        PackageCommandKind::Install,
        &["source", "extra"],
        "exactly one source",
    );
    rejects(PackageCommandKind::Install, &[""], "must not be empty");
    rejects(PackageCommandKind::Update, &[""], "must not be empty");
    rejects(
        PackageCommandKind::Update,
        &["--help", "--bogus"],
        "unrecognized option",
    );
}

#[test]
fn option_values_cannot_be_missing_empty_or_another_option() {
    for option in [
        "--rev",
        "--as",
        "--project",
        "--target",
        "--build-input",
        "--optional-build-input",
    ] {
        for suffix in [vec![], vec![""], vec!["--help"]] {
            let mut arguments = vec!["source", option];
            arguments.extend(suffix);
            rejects(PackageCommandKind::Install, &arguments, "requires");
        }
    }
    for arguments in [vec!["--to"], vec!["--to", ""], vec!["--to", "--resume"]] {
        rejects(PackageCommandKind::Update, &arguments, "requires");
    }
}

#[cfg(any(unix, windows))]
fn non_utf8() -> OsString {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(vec![b'p', 0xff])
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        OsString::from_wide(&[b'p' as u16, 0xd800])
    }
}

#[test]
#[cfg(any(unix, windows))]
fn semantic_arguments_require_utf8() {
    for prefix in [vec![], vec!["--rev"], vec!["--as"], vec!["--target"]] {
        let mut arguments: Vec<_> = prefix.iter().map(OsString::from).collect();
        arguments.push(non_utf8());
        match parse_arguments(PackageCommandKind::Install, arguments.into_iter()) {
            Err(error) => assert!(error.contains("UTF-8"), "{error}"),
            Ok(_) => panic!("accepted non-UTF-8 semantic argument"),
        }
    }
    for prefix in [vec![], vec!["--to"]] {
        let mut arguments: Vec<_> = prefix.iter().map(OsString::from).collect();
        arguments.push(non_utf8());
        assert!(parse_arguments(PackageCommandKind::Update, arguments.into_iter()).is_err());
    }
}
