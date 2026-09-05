use super::*;

mod named;

fn parse(kind: PackageCommandKind, arguments: &[&str]) -> (PackageCommand, PackageCommandOptions) {
    parse_arguments(kind, arguments.iter().map(OsString::from))
        .unwrap_or_else(|error| panic!("{arguments:?}: {error}"))
        .expect("command, not help")
}

fn rejects(kind: PackageCommandKind, arguments: &[&str], expected: &str) {
    match parse_arguments(kind, arguments.iter().map(OsString::from)) {
        Err(error) => assert!(error.contains(expected), "{arguments:?}: {error}"),
        Ok(_) => panic!("accepted invalid arguments {arguments:?}"),
    }
}

#[test]
fn install_preserves_source_and_leaves_defaults_to_manager() {
    for source in [
        "../local package",
        "https://example.invalid/package.git",
        "git@example.invalid:package.git",
        "包",
    ] {
        let (command, options) = parse(PackageCommandKind::Install, &[source]);
        let PackageCommand::Install {
            source: actual,
            revision,
            alias,
            package,
        } = command
        else {
            panic!("expected install");
        };
        assert_eq!(actual, source);
        assert!(revision.is_none());
        assert!(alias.is_none());
        assert!(package.is_none());
        assert_eq!(options.project_root, PathBuf::from("."));
        assert!(options.targets.is_empty());
    }
}

#[test]
fn install_accepts_options_before_and_after_source() {
    let (command, options) = parse(
        PackageCommandKind::Install,
        &[
            "--rev",
            "release/版本",
            "--target",
            "linux_x64",
            "../source",
            "--as",
            "renamed",
            "--project",
            "../my project",
            "--target",
            "macos_arm64",
        ],
    );
    let PackageCommand::Install {
        source,
        revision,
        alias,
        package,
    } = command
    else {
        panic!("expected install");
    };
    assert_eq!(source, "../source");
    assert_eq!(revision.as_deref(), Some("release/版本"));
    assert_eq!(alias.as_deref(), Some("renamed"));
    assert!(package.is_none());
    assert_eq!(options.project_root, PathBuf::from("../my project"));
    assert_eq!(
        options.targets,
        [TargetProfile::LinuxX64, TargetProfile::MacosArm64]
    );
}

#[test]
fn update_without_selections_delegates_all_packages() {
    let (command, options) = parse(PackageCommandKind::Update, &[]);
    let PackageCommand::Update { packages, revision } = command else {
        panic!("expected update");
    };
    assert!(packages.is_empty());
    assert!(revision.is_none());
    assert!(options.targets.is_empty());
    assert_eq!(options.project_root, PathBuf::from("."));
}

#[test]
fn update_preserves_selection_order_and_revision() {
    let (command, options) = parse(
        PackageCommandKind::Update,
        &[
            "first-package",
            "--to",
            "v2",
            "local_alias",
            "--target",
            "windows_x86_64",
            "--project",
            "project",
        ],
    );
    let PackageCommand::Update { packages, revision } = command else {
        panic!("expected update");
    };
    assert_eq!(packages, ["first-package", "local_alias"]);
    assert_eq!(revision.as_deref(), Some("v2"));
    assert_eq!(options.targets, [TargetProfile::WindowsX64]);
    assert_eq!(options.project_root, PathBuf::from("project"));
}

#[test]
fn resume_retains_command_kind_and_project() {
    for kind in [PackageCommandKind::Install, PackageCommandKind::Update] {
        let install = matches!(kind, PackageCommandKind::Install);
        let (command, options) = parse(kind, &["--project", "project", "--resume"]);
        let PackageCommand::Resume { kind } = command else {
            panic!("expected resume");
        };
        assert_eq!(matches!(kind, PackageCommandKind::Install), install);
        assert_eq!(options.project_root, PathBuf::from("project"));
        assert!(options.targets.is_empty());
    }
}

#[test]
fn discard_review_is_explicit_and_has_no_selection() {
    for kind in [PackageCommandKind::Install, PackageCommandKind::Update] {
        let (command, options) = parse(kind, &["--discard-review", "--project", "project"]);
        assert!(matches!(command, PackageCommand::DiscardReview));
        assert_eq!(options.project_root, PathBuf::from("project"));
        assert!(options.targets.is_empty());
    }
}

#[test]
fn help_does_not_require_a_source_or_construct_a_command() {
    for kind in [PackageCommandKind::Install, PackageCommandKind::Update] {
        assert!(
            parse_arguments(kind, [OsString::from("--help")].into_iter())
                .unwrap()
                .is_none()
        );
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
fn duplicate_targets_reject_after_alias_normalization() {
    for arguments in [
        ["--target", "linux_x86_64", "--target", "linux_x86_64"],
        ["--target", "linux_x64", "--target", "linux_x86_64"],
    ] {
        rejects(PackageCommandKind::Update, &arguments, "duplicate target");
    }
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
    for option in ["--rev", "--as", "--project", "--target"] {
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

#[test]
fn review_controls_reject_new_command_inputs_in_either_order() {
    for control in ["--resume", "--discard-review"] {
        for selection in [
            vec!["source"],
            vec!["--rev", "v1"],
            vec!["--as", "alias"],
            vec!["--target", "linux_x86_64"],
        ] {
            let mut before = vec![control];
            before.extend(selection.iter().copied());
            rejects(PackageCommandKind::Install, &before, "allow only --project");
            let mut after = selection;
            after.push(control);
            rejects(PackageCommandKind::Install, &after, "allow only --project");
        }
        for selection in [
            vec!["package"],
            vec!["--to", "v1"],
            vec!["--target", "macos_arm64"],
        ] {
            let mut before = vec![control];
            before.extend(selection.iter().copied());
            rejects(PackageCommandKind::Update, &before, "allow only --project");
            let mut after = selection;
            after.push(control);
            rejects(PackageCommandKind::Update, &after, "allow only --project");
        }
    }
    for arguments in [
        ["--resume", "--discard-review"],
        ["--discard-review", "--resume"],
    ] {
        rejects(
            PackageCommandKind::Install,
            &arguments,
            "cannot be combined",
        );
        rejects(PackageCommandKind::Update, &arguments, "cannot be combined");
    }
}

#[test]
fn manager_outcomes_map_to_command_exit_status() {
    assert_eq!(exit_status(PackageCommandStatus::Published), 0);
    assert_eq!(exit_status(PackageCommandStatus::ReviewDiscarded), 0);
    assert_eq!(exit_status(PackageCommandStatus::ReviewRequired), 3);
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
fn project_paths_preserve_platform_encoding() {
    let path = non_utf8();
    let (_, options) = parse_arguments(
        PackageCommandKind::Install,
        [
            OsString::from("source"),
            OsString::from("--project"),
            path.clone(),
        ]
        .into_iter(),
    )
    .unwrap()
    .expect("install command");
    assert_eq!(options.project_root.as_os_str(), path.as_os_str());
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
