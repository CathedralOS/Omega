use super::*;

#[test]
fn same_policy_source_edits_invalidate_whole_comparisons_and_foreign_rows_reject() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (baseline_sources, baseline_reviews) = candidate(&tree, "baseline");
    let baseline = lock_from_reviews(&baseline_sources, &baseline_reviews);
    source(&tree, ASSUMPTIONS, "");
    let (original_sources, original_reviews) = candidate(&tree, "original");
    let original = compare(
        baseline.target(TARGET),
        &original_sources,
        &original_reviews,
    );
    let decisions = accepting(&original);
    source(
        &tree,
        &format!("// source-only revision\n{ASSUMPTIONS}"),
        "",
    );
    let (updated_sources, updated_reviews) = candidate(&tree, "updated");
    let root = original_sources.graph().root();
    assert_eq!(root, updated_sources.graph().root());
    assert_eq!(
        original_reviews.review(root).unwrap().policy(),
        updated_reviews.review(root).unwrap().policy()
    );
    let updated = compare(baseline.target(TARGET), &updated_sources, &updated_reviews);
    assert_ne!(original.fingerprint(), updated.fingerprint());
    assert_ne!(
        original.candidate_source_subject(),
        updated.candidate_source_subject()
    );
    assert_eq!(
        original.baseline_source_subject(),
        updated.baseline_source_subject()
    );
    assert_eq!(
        resolve_package_policy_decisions(&updated, original.fingerprint().digest(), &decisions),
        Err(Error::WrongComparison)
    );
    let comparison = updated.fingerprint().digest();
    assert_eq!(
        resolve_package_policy_decisions(&updated, comparison, &decisions),
        Err(Error::UnknownSubject(decisions[0].subject))
    );
    let foreign_tree = Tree::new();
    let foreign = initial_assumptions(&foreign_tree);
    let foreign_decisions = accepting(&foreign);
    assert_eq!(
        resolve_package_policy_decisions(&updated, comparison, &foreign_decisions),
        Err(Error::UnknownSubject(foreign_decisions[0].subject))
    );
    assert!(
        resolve_package_policy_decisions(&updated, comparison, &accepting(&updated))
            .unwrap()
            .all_required_changes_accepted()
    );

    source(
        &tree,
        "// baseline source-only revision\npub const VALUE: u64 = 7;\n",
        "",
    );
    let (revised_sources, revised_reviews) = candidate(&tree, "revised-baseline");
    assert_eq!(root, revised_sources.graph().root());
    assert_eq!(
        baseline_reviews.review(root).unwrap().policy(),
        revised_reviews.review(root).unwrap().policy()
    );
    let revised_baseline = lock_from_reviews(&revised_sources, &revised_reviews);
    let revised = compare(
        revised_baseline.target(TARGET),
        &updated_sources,
        &updated_reviews,
    );
    assert_ne!(updated.fingerprint(), revised.fingerprint());
    assert_ne!(
        updated.baseline_source_subject(),
        revised.baseline_source_subject()
    );
    assert_eq!(
        updated.candidate_source_subject(),
        revised.candidate_source_subject()
    );
    assert_eq!(
        resolve_package_policy_decisions(&revised, comparison, &accepting(&updated)),
        Err(Error::WrongComparison)
    );
    assert!(
        resolve_package_policy_decisions(
            &revised,
            revised.fingerprint().digest(),
            &accepting(&revised)
        )
        .unwrap()
        .all_required_changes_accepted()
    );
}

#[test]
fn removed_package_choices_resolve_without_its_old_source_or_cache() {
    let tree = Tree::new();
    source(
        &tree,
        "pub const VALUE: u64 = 7;\n",
        " builder.depend_as(\"dependency\", Source::Path { location: \"../old\" });\n",
    );
    package(&tree.path("sources/old"), "removed-package", "");
    let lock = {
        let (closure, reviews) = candidate(&tree, "accepted");
        lock_from_reviews(&closure, &reviews)
    };
    fs::rename(tree.path("sources/old"), tree.path("unavailable-old")).unwrap();
    fs::rename(tree.path("accepted-cache"), tree.path("unavailable-cache")).unwrap();
    assert!(!tree.path("sources/old").exists());
    assert!(!tree.path("accepted-cache").exists());
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (closure, reviews) = candidate(&tree, "removed");
    let changes = compare(lock.target(TARGET), &closure, &reviews);
    let removed = changes
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "removed-package")
        .unwrap();
    assert!(removed.candidate_resolution().is_none());
    assert!(removed.candidate_path().is_none());
    let row = removed
        .rows()
        .iter()
        .find(|row| row.requires_decision())
        .unwrap();
    assert_eq!(row.change(), PackagePolicyChangeKind::Removed);
    let subject = Subject::Row(row.fingerprint().digest());
    let comparison = changes.fingerprint().digest();
    let mut decisions = accepting(&changes);
    assert!(
        resolve_package_policy_decisions(&changes, comparison, &decisions)
            .unwrap()
            .all_required_changes_accepted()
    );
    let index = decisions
        .iter()
        .position(|decision| decision.subject == subject)
        .unwrap();
    decisions.remove(index);
    assert_eq!(
        resolve_package_policy_decisions(&changes, comparison, &decisions),
        Err(Error::MissingDecision(subject))
    );
    decisions.push(PackagePolicyDecision {
        subject,
        disposition: RejectCandidateChange,
    });
    let resolution = resolve_package_policy_decisions(&changes, comparison, &decisions).unwrap();
    assert!(!resolution.all_required_changes_accepted());
    assert!(resolution.decisions().iter().any(|decision| {
        decision.subject == subject && decision.disposition == RejectCandidateChange
    }));
}

#[test]
fn root_role_choices_are_required_in_both_directions_and_sort_before_rows() {
    let tree = Tree::new();
    let main = "data Main { }\nmachine Main::main(&mut self) { }\n";
    source(&tree, &format!("{main}pub const VALUE: u64 = 7;\n"), "");
    let (package_sources, package_reviews) = candidate(&tree, "package-role");
    let package_lock = lock_from_reviews(&package_sources, &package_reviews);
    fs::write(
        tree.path("sources/root/main.omg"),
        format!("{main}pub const VALUE: u64 = 8;\n"),
    )
    .unwrap();
    fs::write(
        tree.path("sources/root/build.omg"),
        concat!(
            "machine build(builder: &mut Build) {\n",
            " builder.application(\"policy-fixture\");\n",
            " builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);\n}\n",
        ),
    )
    .unwrap();
    let (application_sources, application_reviews) = candidate(&tree, "application-role");
    let application_lock = lock_from_reviews(&application_sources, &application_reviews);
    let to_application = compare(
        package_lock.target(TARGET),
        &application_sources,
        &application_reviews,
    );
    let to_package = compare(
        application_lock.target(TARGET),
        &package_sources,
        &package_reviews,
    );
    for changes in [to_application, to_package] {
        assert!(changes.root_role_change().is_some());
        let comparison = changes.fingerprint().digest();
        let mut decisions = accepting(&changes);
        assert!(decisions.len() >= 2);
        assert_eq!(decisions[0].subject, Subject::RootRole);
        assert_eq!(
            resolve_package_policy_decisions(&changes, comparison, &decisions[1..]),
            Err(Error::MissingDecision(Subject::RootRole))
        );
        let accepted = resolve_package_policy_decisions(&changes, comparison, &decisions).unwrap();
        assert!(accepted.all_required_changes_accepted());
        assert_eq!(accepted.decisions(), decisions);
        decisions[0].disposition = RejectCandidateChange;
        let expected = decisions.clone();
        decisions.reverse();
        let rejected = resolve_package_policy_decisions(&changes, comparison, &decisions).unwrap();
        assert!(!rejected.all_required_changes_accepted());
        assert_eq!(rejected.decisions(), expected);
    }
}
