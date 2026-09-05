use super::*;

#[test]
fn initial_assumption_install_stays_pending_then_rejected_until_accepting_resume() {
    let tree = fixture(ASSUMPTION);
    let before = accepted_files(&tree);
    let initial = pending_install(&tree);
    assert!(initial.report.contains("Audit recommended"));
    let findings = documents(&initial);
    assert!(findings[0].starts_with("omega-package-review 1\n"));
    assert!(
        findings[0]
            .lines()
            .any(|line| line.starts_with("decision ") && line.ends_with(" pending"))
    );
    let proposal = fs::read(proposal_path(&tree)).unwrap();

    let pending = resume(&tree, PackageCommandKind::Install).unwrap();
    assert_eq!(pending.status, PackageCommandStatus::ReviewRequired);
    assert_eq!(documents(&pending), findings);
    assert_eq!(accepted_files(&tree), before);
    assert_eq!(fs::read(proposal_path(&tree)).unwrap(), proposal);

    assert!(edit_decisions(&initial.review_paths[0], "reject") > 0);
    let rejected_document = documents(&initial);
    let rejected = resume(&tree, PackageCommandKind::Install).unwrap();
    assert_eq!(rejected.status, PackageCommandStatus::ReviewRequired);
    assert_eq!(documents(&rejected), rejected_document);
    assert_eq!(accepted_files(&tree), before);
    assert_eq!(fs::read(proposal_path(&tree)).unwrap(), proposal);

    accept(&initial);
    let published = resume(&tree, PackageCommandKind::Install).unwrap();
    assert_eq!(
        published.status,
        PackageCommandStatus::Published,
        "{}",
        published.report
    );
    assert!(!proposal_path(&tree).exists());
    let after = accepted_files(&tree);
    assert_ne!(after.0, before.0);
    assert!(
        String::from_utf8(after.0)
            .unwrap()
            .contains("../dependency")
    );
    let accepted = lock(&tree);
    assert_eq!(accepted.targets().len(), 1);
    assert_eq!(
        accepted.target(TARGET).unwrap().source().packages().len(),
        2
    );
    assert!(
        !accepted
            .target(TARGET)
            .unwrap()
            .decisions()
            .decisions()
            .is_empty()
    );
}

#[test]
fn fresh_commands_and_wrong_resume_kind_preserve_existing_proposal() {
    let tree = fixture(ASSUMPTION);
    let initial = pending_install(&tree);
    let before = accepted_files(&tree);
    let proposal = fs::read(proposal_path(&tree)).unwrap();
    let findings = documents(&initial);
    for command in [install(), update()] {
        let error = execute(&tree, command, vec![TARGET])
            .unwrap_err()
            .to_string();
        assert!(error.contains("pending"), "{error}");
        assert_eq!(accepted_files(&tree), before);
        assert_eq!(fs::read(proposal_path(&tree)).unwrap(), proposal);
        assert_eq!(documents(&initial), findings);
    }
    let error = resume(&tree, PackageCommandKind::Update)
        .unwrap_err()
        .to_string();
    assert!(error.contains("other package command"), "{error}");
    assert_eq!(accepted_files(&tree), before);
    assert_eq!(fs::read(proposal_path(&tree)).unwrap(), proposal);
}

#[test]
fn changed_findings_cannot_be_accepted_by_editing_decision_tokens() {
    let tree = fixture(ASSUMPTION);
    let initial = pending_install(&tree);
    accept(&initial);
    let before = accepted_files(&tree);
    let path = &initial.review_paths[0];
    let text = fs::read_to_string(path).unwrap();
    assert!(text.contains("command-dependency"));
    let changed = text.replacen("command-dependency", "foreign-dependency", 1);
    fs::write(path, &changed).unwrap();
    let error = resume(&tree, PackageCommandKind::Install)
        .unwrap_err()
        .to_string();
    assert!(error.contains("cannot resume"), "{error}");
    assert_eq!(fs::read_to_string(path).unwrap(), changed);
    assert_eq!(accepted_files(&tree), before);
    assert!(proposal_path(&tree).exists());
}

#[test]
fn missing_review_document_reports_its_name_and_preserves_project_files() {
    let tree = fixture(ASSUMPTION);
    let initial = pending_install(&tree);
    let before = accepted_files(&tree);
    let path = &initial.review_paths[0];
    // Test-owned temporary findings only; accepted files remain untouched.
    fs::remove_file(path).unwrap();
    let error = resume(&tree, PackageCommandKind::Install)
        .unwrap_err()
        .to_string();
    assert!(error.contains("missing review document"), "{error}");
    assert!(
        error.contains(path.file_name().unwrap().to_str().unwrap()),
        "{error}"
    );
    assert!(!path.exists());
    assert_eq!(accepted_files(&tree), before);
    assert!(proposal_path(&tree).exists());
}

#[test]
fn discard_abandons_only_pending_review_and_never_rolls_back_accepted_files() {
    let tree = fixture(ASSUMPTION);
    assert_eq!(
        execute(&tree, update(), vec![TARGET]).unwrap().status,
        PackageCommandStatus::Published
    );
    let before = accepted_files(&tree);
    assert!(before.1.is_some());
    let initial = pending_install(&tree);
    for _ in 0..2 {
        let discarded = execute(&tree, PackageCommand::DiscardReview, Vec::new()).unwrap();
        assert_eq!(discarded.status, PackageCommandStatus::ReviewDiscarded);
        assert!(!proposal_path(&tree).exists());
        assert_eq!(accepted_files(&tree), before);
    }
    let error = resume(&tree, PackageCommandKind::Install)
        .unwrap_err()
        .to_string();
    assert!(error.contains("no pending package review"), "{error}");
    assert_eq!(accepted_files(&tree), before);
    let restarted = pending_install(&tree);
    assert_eq!(restarted.review_paths, initial.review_paths);
    accept(&restarted);
    assert_eq!(
        resume(&tree, PackageCommandKind::Install).unwrap().status,
        PackageCommandStatus::Published
    );
    let after = accepted_files(&tree);
    assert_ne!(after, before);
    execute(&tree, PackageCommand::DiscardReview, Vec::new()).unwrap();
    assert_eq!(accepted_files(&tree), after);
}

#[test]
fn retained_assumptions_recommend_audit_for_repeated_source_upgrades() {
    repeated_update_audit(ASSUMPTION);
}

#[test]
fn retained_external_supply_recommends_audit_for_repeated_source_upgrades() {
    repeated_update_audit(concat!(
        "pub boundary trait ForeignSurface { machine invoke() reaches ForeignSurface; }\n",
        "pub machine invoke_leaf() satisfies ForeignSurface::invoke\n",
        " via Binding::DllImport(\"omega-host\", \"invoke\");\n",
    ));
}

#[test]
fn retained_filesystem_authority_recommends_audit_for_repeated_source_upgrades() {
    // Same source as policy_changes/authority.rs. The command's candidate
    // entrance discovers and recompiles the exact FilesystemHost binding.
    let accepted = repeated_update_audit(concat!(
        "pub boundary trait FilesystemHost {\n",
        "    machine inspect() reaches FilesystemHost;\n",
        "}\n",
        "pub machine access()\n",
        "    reaches FilesystemHost\n",
        "    invokes FilesystemHost;\n",
        "{ FilesystemHost::inspect(); }\n",
    ));
    assert!(
        accepted
            .target(TARGET)
            .unwrap()
            .baselines()
            .iter()
            .any(|baseline| { !baseline.dangerous_capabilities().is_empty() })
    );
}

#[test]
fn update_checks_changed_dependency_and_publishes_only_after_update_resume() {
    let tree = fixture(PURE);
    assert_eq!(
        execute(&tree, install(), vec![TARGET]).unwrap().status,
        PackageCommandStatus::Published
    );
    let accepted = lock(&tree);
    let before = accepted_files(&tree);
    fs::write(
        tree.path("sources/dependency/main.omg"),
        format!("{PURE}{ASSUMPTION}"),
    )
    .unwrap();
    let pending = execute(&tree, update(), Vec::new()).unwrap();
    assert_eq!(pending.status, PackageCommandStatus::ReviewRequired);
    assert_eq!(accepted_files(&tree), before);
    accept(&pending);
    assert!(resume(&tree, PackageCommandKind::Install).is_err());
    assert_eq!(accepted_files(&tree), before);
    let published = resume(&tree, PackageCommandKind::Update).unwrap();
    assert_eq!(published.status, PackageCommandStatus::Published);
    let current = lock(&tree);
    assert_ne!(
        current.target(TARGET).unwrap().source(),
        accepted.target(TARGET).unwrap().source()
    );
    assert_eq!(accepted_files(&tree).0, before.0);
    assert_ne!(accepted_files(&tree).1, before.1);
    assert!(!proposal_path(&tree).exists());
}

fn repeated_update_audit(dependency: &str) -> PackageLock {
    let tree = fixture(dependency);
    let initial = pending_install(&tree);
    accept(&initial);
    assert_eq!(
        resume(&tree, PackageCommandKind::Install).unwrap().status,
        PackageCommandStatus::Published
    );
    let accepted = lock(&tree);
    let mut previous = accepted.clone();
    for revision in 1..=2 {
        fs::write(
            tree.path("sources/dependency/main.omg"),
            format!("// dependency source revision {revision}\n{dependency}"),
        )
        .unwrap();
        let updated = execute(&tree, update(), Vec::new()).unwrap();
        assert_eq!(
            updated.status,
            PackageCommandStatus::Published,
            "{}",
            updated.report
        );
        assert!(
            updated
                .report
                .contains("Audit recommended: command-dependency"),
            "{}",
            updated.report
        );
        let current = lock(&tree);
        let old_source = previous.target(TARGET).unwrap().source();
        let new_source = current.target(TARGET).unwrap().source();
        let old_dependency = old_source
            .packages()
            .iter()
            .find(|package| package.key().name().as_str() == "command-dependency")
            .unwrap();
        let new_dependency = new_source
            .packages()
            .iter()
            .find(|package| package.key().name().as_str() == "command-dependency")
            .unwrap();
        assert_eq!(old_dependency.key(), new_dependency.key());
        assert_ne!(old_dependency.resolution(), new_dependency.resolution());
        assert_eq!(old_source.root().selected(), new_source.root().selected());
        assert_eq!(
            current.target(TARGET).unwrap().baselines(),
            accepted.target(TARGET).unwrap().baselines()
        );
        assert!(!proposal_path(&tree).exists());
        previous = current;
    }
    accepted
}
