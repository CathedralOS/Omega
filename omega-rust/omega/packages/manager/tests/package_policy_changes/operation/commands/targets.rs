use super::*;
use package_manager::operations::{LockedSourceRecoveryOptions, check_locked_sources};

#[test]
fn every_retained_target_requires_choices_and_survives_subset_updates() {
    let tree = fixture(ASSUMPTION);
    let targets = vec![TARGET, TargetProfile::LinuxArm64];
    let baseline = execute(&tree, update(), targets).unwrap();
    assert_eq!(baseline.status, PackageCommandStatus::Published);
    assert_eq!(lock(&tree).targets().len(), 2);
    let before = accepted_files(&tree);
    let initial = execute(&tree, install(), vec![TARGET]).unwrap();
    assert_eq!(initial.status, PackageCommandStatus::ReviewRequired);
    assert_eq!(initial.review_paths.len(), 2);
    assert_eq!(accepted_files(&tree), before);
    assert!(
        initial.review_paths[0]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("linux_arm64")
    );
    assert!(edit_decisions(&initial.review_paths[0], "accept") > 0);
    let partial = resume(&tree, PackageCommandKind::Install).unwrap();
    assert_eq!(partial.status, PackageCommandStatus::ReviewRequired);
    assert_eq!(accepted_files(&tree), before);
    assert!(proposal_path(&tree).exists());
    assert!(edit_decisions(&initial.review_paths[1], "accept") > 0);
    let published = resume(&tree, PackageCommandKind::Install).unwrap();
    assert_eq!(published.status, PackageCommandStatus::Published);
    let accepted = lock(&tree);
    assert_eq!(accepted.targets().len(), 2);
    assert_eq!(accepted.targets()[0].target(), TargetProfile::LinuxArm64);
    assert_eq!(accepted.targets()[1].target(), TARGET);
    for target in accepted.targets() {
        assert_eq!(target.source().packages().len(), 2);
    }
    let updated = execute(&tree, update(), vec![TARGET]).unwrap();
    assert_eq!(updated.status, PackageCommandStatus::Published);
    let current = lock(&tree);
    assert_eq!(current.targets().len(), 2);
    for target in accepted.targets() {
        assert_eq!(
            current.target(target.target()).unwrap().source(),
            target.source()
        );
    }
}

#[test]
fn resume_cannot_change_pending_target_coverage() {
    let tree = fixture(ASSUMPTION);
    let initial = pending_install(&tree);
    accept(&initial);
    let before = accepted_files(&tree);
    let error = execute(
        &tree,
        PackageCommand::Resume {
            kind: PackageCommandKind::Install,
        },
        vec![TargetProfile::LinuxArm64],
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("pending proposal's targets"), "{error}");
    assert_eq!(accepted_files(&tree), before);
    assert!(proposal_path(&tree).exists());
}

#[test]
fn pure_initial_install_and_unchanged_update_reload_exact_pins_and_compile_again() {
    let tree = fixture(PURE);
    // An import forces the proposed declaration to take part in real checking.
    fs::write(
        tree.path("sources/root/main.omg"),
        "use dependency::main;\npub machine value() -> u64 { VALUE }\n",
    )
    .unwrap();
    let before = accepted_files(&tree);
    let published = execute(&tree, install(), vec![TARGET]).unwrap();
    assert_eq!(
        published.status,
        PackageCommandStatus::Published,
        "{}",
        published.report
    );
    assert_ne!(accepted_files(&tree).0, before.0);
    assert!(!proposal_path(&tree).exists());
    let accepted = lock(&tree);
    let updated = execute(&tree, update(), Vec::new()).unwrap();
    assert_eq!(
        updated.status,
        PackageCommandStatus::Published,
        "{}",
        updated.report
    );
    let current = lock(&tree);
    assert_eq!(
        current.target(TARGET).unwrap().source(),
        accepted.target(TARGET).unwrap().source()
    );
    assert_eq!(
        current.target(TARGET).unwrap().baselines(),
        accepted.target(TARGET).unwrap().baselines()
    );
    let recovered = resolve_external_local_project_closure_with_storage(
        fs::canonicalize(tree.path("sources/root")).unwrap(),
        ExternalSourceContext::derive(b"omega-local-project-v1"),
        &tree.storage("fresh-command-cache"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .unwrap();
    assert_fresh_matches(&current, &recovered);
    let checked = check_locked_sources(
        &current,
        TARGET,
        recovered.source_requests().root().request(),
        &tree.storage("fresh-check-cache"),
        LockedSourceRecoveryOptions::default(),
        &tree.path("fresh-command-check"),
    )
    .unwrap();
    assert!(checked.changed_policies().is_empty());
    assert_fresh_matches(&current, checked.source_closure());
}
