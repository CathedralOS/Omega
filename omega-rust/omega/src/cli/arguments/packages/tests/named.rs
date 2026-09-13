use super::*;

#[test]
fn named_git_install_preserves_selection_separately_from_alias() {
    for arguments in [
        vec![
            "--package",
            "exact-math",
            "https://example.invalid/workspace.git",
        ],
        vec![
            "git@example.invalid:workspace.git",
            "--package",
            "exact-math",
            "--as",
            "math",
        ],
    ] {
        let (command, _) = parse(PackageCommandKind::Install, &arguments);
        let PackageCommand::Install { package, alias, .. } = command else {
            panic!("install");
        };
        assert_eq!(package.as_deref(), Some("exact-math"));
        assert_eq!(
            alias.as_deref(),
            arguments.contains(&"--as").then_some("math")
        );
    }
}

#[test]
fn package_selection_rejects_duplicates_missing_values_and_wrong_commands() {
    rejects(
        PackageCommandKind::Install,
        &["source", "--package", "one", "--package", "two"],
        "duplicate --package",
    );
    for suffix in [vec![], vec![""], vec!["--as"]] {
        let mut arguments = vec!["source", "--package"];
        arguments.extend(suffix);
        rejects(PackageCommandKind::Install, &arguments, "requires");
    }
    rejects(
        PackageCommandKind::Update,
        &["--package", "one"],
        "only valid for install",
    );
}

#[test]
fn review_controls_cannot_replace_a_retained_package_selection() {
    for control in ["--resume", "--discard-review"] {
        for arguments in [
            [control, "--package", "exact-math"],
            ["--package", "exact-math", control],
        ] {
            rejects(
                PackageCommandKind::Install,
                &arguments,
                "allow only --project",
            );
        }
    }
}

#[test]
#[cfg(any(unix, windows))]
fn declared_package_name_requires_utf8() {
    let result = parse_arguments(
        PackageCommandKind::Install,
        [
            OsString::from("source"),
            OsString::from("--package"),
            non_utf8(),
        ]
        .into_iter(),
    );
    assert!(matches!(result, Err(error) if error.contains("UTF-8")));
}

#[test]
fn install_help_describes_declared_name_selection() {
    let text = usage(&PackageCommandKind::Install);
    assert!(text.contains("--package <declared-name>"));
    assert!(text.contains("Git workspace member"));
    assert!(!usage(&PackageCommandKind::Update).contains("--package"));
}
