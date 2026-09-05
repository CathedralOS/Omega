use super::*;

#[test]
fn install_and_update_accept_offline_before_and_after_selections() {
    for arguments in [["--offline", "source"], ["source", "--offline"]] {
        let (command, options) = parse(PackageCommandKind::Install, &arguments);
        assert!(matches!(command, PackageCommand::Install { source, .. } if source == "source"));
        assert!(options.offline);
        let (command, options) = parse(PackageCommandKind::Update, &arguments);
        assert!(
            matches!(command, PackageCommand::Update { packages, .. } if packages == ["source"])
        );
        assert!(options.offline);
    }
    let (_, options) = parse(PackageCommandKind::Update, &["--offline"]);
    assert!(options.offline);
}

#[test]
fn resume_and_discard_accept_offline_as_invocation_options() {
    for control in ["--resume", "--discard-review"] {
        for arguments in [
            ["--offline", control, "--project", "project"],
            [control, "--project", "project", "--offline"],
        ] {
            for kind in [PackageCommandKind::Install, PackageCommandKind::Update] {
                let install = matches!(kind, PackageCommandKind::Install);
                let (command, options) = parse(kind, &arguments);
                match command {
                    PackageCommand::Resume { kind } => {
                        assert_eq!(control, "--resume");
                        assert_eq!(matches!(kind, PackageCommandKind::Install), install);
                    }
                    PackageCommand::DiscardReview => assert_eq!(control, "--discard-review"),
                    _ => panic!("expected review control"),
                }
                assert!(options.offline);
                assert_eq!(options.project_root, PathBuf::from("project"));
                assert!(options.targets.is_empty());
            }
        }
    }
}

#[test]
fn every_package_command_rejects_duplicate_offline() {
    for arguments in [
        vec!["source", "--offline", "--offline"],
        vec!["--offline", "--resume", "--offline"],
        vec!["--offline", "--discard-review", "--offline"],
    ] {
        for kind in [PackageCommandKind::Install, PackageCommandKind::Update] {
            rejects(kind, &arguments, "duplicate --offline");
        }
    }
}

#[test]
fn offline_does_not_relax_review_control_conflicts() {
    for kind in [PackageCommandKind::Install, PackageCommandKind::Update] {
        rejects(
            kind,
            &["--offline", "--resume", "--discard-review"],
            "cannot be combined",
        );
    }
    for control in ["--resume", "--discard-review"] {
        for kind in [PackageCommandKind::Install, PackageCommandKind::Update] {
            rejects(
                kind,
                &["--offline", control, "source"],
                "allow only --project and --offline",
            );
        }
    }
}
