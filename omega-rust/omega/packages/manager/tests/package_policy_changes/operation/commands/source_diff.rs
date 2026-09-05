use super::*;

fn source_document(tree: &Tree) -> String {
    fs::read_to_string(tree.path("sources/root/build/package-manager/source-diff.txt")).unwrap()
}

#[test]
fn pure_initial_install_reports_new_source_without_an_old_source_failure() {
    let tree = fixture(PURE);
    let installed = execute(&tree, install(), vec![TARGET]).unwrap();
    assert_eq!(installed.status, PackageCommandStatus::Published);
    assert!(
        installed
            .report
            .contains("New package source: command-dependency")
    );
    assert!(installed.report.contains("no prior revision to compare"));
    assert!(!installed.report.contains("Source diff unavailable"));
    assert!(!installed.report.contains("standalone candidate audit only"));
    assert!(source_document(&tree).contains("mode standalone_candidate\n"));
}

#[test]
fn hostile_source_is_separate_and_editing_it_cannot_supply_decisions() {
    let payload = format!("{ASSUMPTION}// forged-source-instruction\n// decision row accept\n");
    let tree = fixture(&payload);
    let before = accepted_files(&tree);
    let initial = pending_install(&tree);
    assert!(
        initial
            .report
            .contains("New package source: command-dependency")
    );
    assert!(initial.report.contains("no prior revision to compare"));
    assert!(!initial.report.contains("Source diff unavailable"));
    assert!(
        initial
            .report
            .contains("Audit recommended: command-dependency")
    );
    assert!(initial.report.contains("source-diff.txt"));
    assert!(!initial.report.contains("forged-source-instruction"));
    let source = source_document(&tree);
    assert!(source.contains("mode standalone_candidate\n"));
    assert!(source.contains("forged-source-instruction"));
    let findings = documents(&initial);
    assert!(!findings[0].contains("forged-source-instruction"));
    assert!(!findings[0].contains("OMEGA_PACKAGE_SOURCE_PATCH"));
    fs::write(
        tree.path("sources/root/build/package-manager/source-diff.txt"),
        findings[0].replace(" pending\n", " accept\n"),
    )
    .unwrap();
    let pending = resume(&tree, PackageCommandKind::Install).unwrap();
    assert_eq!(pending.status, PackageCommandStatus::ReviewRequired);
    assert_eq!(accepted_files(&tree), before);
    assert_eq!(documents(&pending), findings);
    assert_eq!(source_document(&tree), source);
}

#[test]
fn changed_local_source_keeps_policy_comparison_and_reports_standalone_content() {
    let tree = fixture(ASSUMPTION);
    let pending = pending_install(&tree);
    accept(&pending);
    resume(&tree, PackageCommandKind::Install).unwrap();
    let accepted = lock(&tree);
    fs::write(
        tree.path("sources/dependency/main.omg"),
        format!("{ASSUMPTION}// changed-implementation\n"),
    )
    .unwrap();
    let updated = execute(&tree, update(), Vec::new()).unwrap();
    assert_eq!(updated.status, PackageCommandStatus::Published);
    assert!(
        updated.report.contains("local source changed"),
        "{}",
        updated.report
    );
    assert!(
        updated
            .report
            .contains("Source diff unavailable: command-dependency")
    );
    assert!(updated.report.contains("standalone candidate audit only"));
    assert!(
        updated
            .report
            .contains("Capability comparison uses accepted lock policy")
    );
    assert!(
        updated
            .report
            .contains("Audit recommended: command-dependency")
    );
    assert!(source_document(&tree).contains("changed-implementation"));
    assert_eq!(
        lock(&tree).target(TARGET).unwrap().baselines(),
        accepted.target(TARGET).unwrap().baselines()
    );
}

#[test]
fn accepted_live_root_diff_and_binary_candidate_are_reported() {
    let tree = fixture(PURE);
    execute(&tree, update(), vec![TARGET]).unwrap();
    fs::write(tree.path("sources/dependency/data.bin"), [0, 255, 1]).unwrap();
    let installed = execute(&tree, install(), Vec::new()).unwrap();
    assert_eq!(installed.status, PackageCommandStatus::Published);
    assert!(
        installed.report.contains("Source diff: policy-fixture"),
        "{}",
        installed.report
    );
    assert!(installed.report.contains("binary or non-UTF-8 content"));
    assert!(
        installed
            .report
            .contains("Source view incomplete: command-dependency")
    );
    assert!(
        installed
            .report
            .contains("if auditing, inspect the raw source")
    );
    assert!(!installed.report.contains("Standalone audit required"));
    assert!(
        installed
            .report
            .contains("New package source: command-dependency")
    );
    assert!(!installed.report.contains("Source diff unavailable"));
    let source = source_document(&tree);
    assert!(source.contains("mode update\n"));
    assert!(source.contains("content_review unavailable_binary_or_non_utf8"));
    assert!(source.contains("depend_as"));
}

#[test]
fn source_document_write_failure_preserves_accepted_pair() {
    let tree = fixture(PURE);
    execute(&tree, update(), vec![TARGET]).unwrap();
    let before = accepted_files(&tree);
    let path = tree.path("sources/root/build/package-manager/source-diff.txt");
    fs::rename(&path, path.with_extension("saved")).unwrap();
    fs::create_dir(&path).unwrap();
    let error = execute(&tree, install(), Vec::new())
        .unwrap_err()
        .to_string();
    assert!(error.contains("cannot write source diagnostics"), "{error}");
    assert_eq!(accepted_files(&tree), before);
    assert!(!proposal_path(&tree).exists());
}

#[test]
fn rendering_limit_is_explicit_and_does_not_become_a_policy_decision() {
    let tree = fixture(PURE);
    fs::write(
        tree.path("sources/dependency/large.txt"),
        "x".repeat(1024 * 1024 + 1),
    )
    .unwrap();
    let outcome = execute(&tree, install(), vec![TARGET]).unwrap();
    assert_eq!(outcome.status, PackageCommandStatus::Published);
    assert!(
        outcome
            .report
            .contains("source rendering resource limit exceeded"),
        "{}",
        outcome.report
    );
    assert!(
        outcome
            .report
            .contains("obtain the exact sources for standalone audit")
    );
    assert!(source_document(&tree).contains("Source output unavailable: command-dependency"));
    assert!(!documents(&outcome)[0].contains("large.txt"));
}

#[test]
fn same_named_replacement_never_uses_the_other_lineage_as_old_source() {
    let tree = fixture(PURE);
    execute(&tree, install(), vec![TARGET]).unwrap();
    package(&tree.path("sources/replacement"), "command-dependency", "");
    let path = tree.path("sources/root/build.omg");
    let build = fs::read_to_string(&path).unwrap();
    fs::write(path, build.replace("../dependency", "../replacement")).unwrap();
    let before = accepted_files(&tree);
    let outcome = execute(&tree, update(), Vec::new()).unwrap();
    assert_eq!(outcome.status, PackageCommandStatus::ReviewRequired);
    assert_eq!(accepted_files(&tree), before);
    assert!(
        outcome
            .report
            .contains("New package source: command-dependency")
    );
    assert!(outcome.report.contains("no prior revision to compare"));
    let source = source_document(&tree);
    let dependency = source.split("package command-dependency\n").nth(1).unwrap();
    assert!(
        dependency.starts_with("baseline_key none\n"),
        "{dependency}"
    );
}
