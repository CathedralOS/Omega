use super::*;
use omega_package_manager::declarations::{AliasName, PackageKey};
use omega_package_manager::review::{
    PackagePolicyChangeSet, PackagePolicyDecision, PackagePolicyDecisionError as Error,
    PackagePolicyDecisionSubject as Subject, PackagePolicyReplacementSite as Site,
    ReviewOnlyRootPolicyDisposition::*, resolve_package_policy_decisions,
};

fn empty_package(tree: &Tree, directory: &str, name: &str, dependencies: &[(&str, &str)]) {
    let build: String = dependencies
        .iter()
        .map(|(alias, location)| {
            format!(
                " builder.depend_as(\"{alias}\", Source::Path {{ location: \"../{location}\" }});\n"
            )
        })
        .collect();
    let path = tree.path(&format!("sources/{directory}"));
    package(&path, name, &build);
    fs::write(path.join("main.omg"), "").unwrap();
}

fn baseline(tree: &Tree) -> PackageLock {
    let (closure, reviews) = candidate(tree, "baseline");
    lock_from_reviews(&closure, &reviews)
}

fn accepted_source(lock: &PackageLock) -> &CanonicalSourceClosureSubject {
    lock.target(TARGET).unwrap().source()
}

fn compare(tree: &Tree, label: &str, lock: Option<&PackageLock>) -> PackagePolicyChangeSet {
    let (closure, reviews) = candidate(tree, label);
    let compare = |maximum_changed_rows| {
        compare_package_policy_changes(
            lock.and_then(|lock| lock.target(TARGET)),
            &reviews,
            &closure.for_exact_target(TARGET),
            PackagePolicyChangeLimits {
                maximum_changed_rows,
                ..Default::default()
            },
        )
    };
    let changes = compare(PackagePolicyChangeLimits::default().maximum_changed_rows).unwrap();
    assert!(changes.root_role_change().is_none());
    let count = changes.source_replacements().len()
        + changes
            .packages()
            .iter()
            .map(|package| package.rows().len())
            .sum::<usize>();
    // Compiler/build policy rows and replacements share one change-row budget.
    assert_eq!(compare(count).unwrap(), changes);
    if count > 0 {
        assert!(compare(count - 1).is_err());
    }
    changes
}

fn assert_binding(
    tree: &Tree,
    changes: &PackagePolicyChangeSet,
    lock: &PackageLock,
    requester: &PackageKey,
    alias: &str,
) {
    let site = Site::Dependency {
        requester: requester.clone(),
        alias: AliasName::parse(alias).unwrap(),
    };
    let replacement = changes
        .source_replacements()
        .iter()
        .find(|replacement| replacement.site() == &site)
        .unwrap();
    let old = accepted_source(lock)
        .dependency_requests()
        .iter()
        .find(|selection| selection.requester() == requester && selection.alias().as_str() == alias)
        .unwrap();
    assert_eq!(replacement.baseline(), old.selected().key());
    assert_ne!(replacement.baseline(), replacement.candidate());
    let closure = resolve(tree, &format!("binding-{alias}"));
    let node = closure
        .graph()
        .packages()
        .iter()
        .find(|node| node.source().key() == requester)
        .unwrap();
    let selected = node
        .dependencies()
        .iter()
        .find(|dependency| dependency.alias().as_str() == alias)
        .unwrap();
    assert_eq!(replacement.candidate(), selected.target());
}

#[test]
fn root_rename_and_source_replacement_have_distinct_source_findings() {
    for rename in [true, false] {
        let old = Tree::new();
        let other = Tree::new();
        empty_package(&old, "root", "original", &[("service", "old")]);
        empty_package(&old, "old", "service", &[]);
        let lock = baseline(&old);
        let next = if rename { &old } else { &other };
        empty_package(
            next,
            "root",
            if rename { "renamed" } else { "original" },
            &[("service", "new")],
        );
        empty_package(next, "new", "service", &[]);
        let changes = compare(next, "replacement", Some(&lock));
        assert!(changes.root_changed());
        assert!(changes.requires_decision());
        // A changed requester cannot pair its dependency with the former root's alias.
        assert_eq!(changes.source_replacements().len(), 1);
        let replacement = &changes.source_replacements()[0];
        assert_eq!(replacement.site(), &Site::Root);
        assert_eq!(
            replacement.baseline(),
            accepted_source(&lock).root().selected().key()
        );
        assert_eq!(
            replacement.candidate(),
            resolve(next, "root-identity").graph().root()
        );
        assert_ne!(replacement.baseline(), replacement.candidate());
    }
}

#[test]
fn revisions_reordering_alias_changes_and_unrelated_same_name_edges_are_not_replacements() {
    let tree = Tree::new();
    for directory in ["old", "other", "new"] {
        empty_package(&tree, directory, "same-name", &[]);
    }
    empty_package(
        &tree,
        "root",
        "root",
        &[("left", "old"), ("right", "other")],
    );
    let initial = compare(&tree, "initial", None);
    assert!(initial.source_replacements().is_empty());
    let lock = baseline(&tree);
    let unchanged = compare(&tree, "unchanged", Some(&lock));
    assert!(!unchanged.source_subject_changed());
    for (label, dependencies) in [
        ("reordered", [("right", "other"), ("left", "old")]),
        ("alias-only", [("renamed", "old"), ("right", "other")]),
        ("independent", [("added", "new"), ("right", "other")]),
        ("revision", [("left", "old"), ("right", "other")]),
    ] {
        empty_package(&tree, "root", "root", &dependencies);
        if label == "revision" {
            fs::write(
                tree.path("sources/old/main.omg"),
                "// same package key, fresh content\n",
            )
            .unwrap();
        }
        let changes = compare(&tree, label, Some(&lock));
        assert!(changes.source_subject_changed(), "{label}");
        assert!(changes.source_replacements().is_empty(), "{label}");
        if label != "independent" {
            assert!(!changes.requires_decision(), "{label}");
            assert!(
                resolve_package_policy_decisions(&changes, changes.fingerprint().digest(), &[])
                    .unwrap()
                    .all_required_changes_accepted()
            );
        }
    }
}

#[test]
fn transitive_replacement_belongs_to_its_exact_requester_and_alias() {
    let tree = Tree::new();
    empty_package(
        &tree,
        "root",
        "root",
        &[
            ("service", "bridge"),
            ("old_available", "old"),
            ("new_available", "new"),
        ],
    );
    empty_package(&tree, "bridge", "bridge", &[("service", "old")]);
    empty_package(&tree, "old", "old-service", &[]);
    empty_package(&tree, "new", "new-service", &[]);
    let lock = baseline(&tree);
    let requester = accepted_source(&lock)
        .dependency_requests()
        .iter()
        .find(|selection| selection.selected().key().name().as_str() == "bridge")
        .unwrap()
        .selected()
        .key()
        .clone();
    empty_package(&tree, "bridge", "bridge", &[("service", "new")]);
    empty_package(&tree, "new", "new-service", &[]);
    let changes = compare(&tree, "transitive", Some(&lock));
    assert_eq!(changes.source_replacements().len(), 1);
    assert!(
        changes
            .packages()
            .iter()
            .all(|package| package.rows().is_empty())
    );
    assert!(changes.requires_decision());
    assert_binding(&tree, &changes, &lock, &requester, "service");
    assert_eq!(
        changes.source_replacements()[0].candidate().name().as_str(),
        "new-service"
    );
}

#[test]
fn reordered_alias_replacements_require_exact_complete_decisions_and_reject_stale_choices() {
    let tree = Tree::new();
    for (directory, name) in [
        ("old", "same-name"),
        ("new", "same-name"),
        ("other", "different-name"),
    ] {
        empty_package(&tree, directory, name, &[]);
    }
    empty_package(
        &tree,
        "root",
        "root",
        &[
            ("left", "old"),
            ("right", "new"),
            ("keep_old", "old"),
            ("keep_other", "other"),
        ],
    );
    let lock = baseline(&tree);
    // Both selections change, including one declared name; authored ordinals also move.
    empty_package(
        &tree,
        "root",
        "root",
        &[
            ("right", "other"),
            ("left", "new"),
            ("keep_old", "old"),
            ("keep_other", "other"),
        ],
    );
    let changes = compare(&tree, "replacements", Some(&lock));
    assert_eq!(changes.source_replacements().len(), 2);
    assert!(
        changes
            .packages()
            .iter()
            .all(|package| package.rows().is_empty())
    );
    assert!(changes.requires_decision());
    let requester = accepted_source(&lock).root().selected().key();
    for (alias, name) in [("left", "same-name"), ("right", "different-name")] {
        assert_binding(&tree, &changes, &lock, requester, alias);
        let replacement = changes.source_replacements().iter().find(|replacement| {
            matches!(replacement.site(), Site::Dependency { alias: found, .. } if found.as_str() == alias)
        }).unwrap();
        assert_eq!(replacement.candidate().name().as_str(), name);
    }
    let comparison = changes.fingerprint().digest();
    let mut decisions: Vec<_> = changes
        .source_replacements()
        .iter()
        .map(|replacement| PackagePolicyDecision {
            subject: Subject::SourceReplacement(replacement.fingerprint().digest()),
            disposition: AcceptCandidateChange,
        })
        .collect();
    decisions.sort_by_key(|decision| decision.subject);
    assert_ne!(decisions[0].subject, decisions[1].subject);
    let mut reversed = decisions.clone();
    reversed.reverse();
    let accepted = resolve_package_policy_decisions(&changes, comparison, &reversed).unwrap();
    assert_eq!(accepted.decisions(), decisions);
    assert_eq!(accepted.comparison(), changes.fingerprint());
    assert!(accepted.all_required_changes_accepted());
    assert_eq!(
        resolve_package_policy_decisions(&changes, comparison, &[]),
        Err(Error::MissingDecision(decisions[0].subject))
    );
    for index in 0..decisions.len() {
        let remaining = [decisions[1 - index]];
        assert_eq!(
            resolve_package_policy_decisions(&changes, comparison, &remaining),
            Err(Error::MissingDecision(decisions[index].subject))
        );
        let mut rejected = decisions.clone();
        rejected[index].disposition = RejectCandidateChange;
        let resolution = resolve_package_policy_decisions(&changes, comparison, &rejected).unwrap();
        assert_eq!(resolution.decisions(), rejected);
        assert!(!resolution.all_required_changes_accepted());
    }
    assert_eq!(
        resolve_package_policy_decisions(&changes, comparison, &[decisions[0], decisions[0]]),
        Err(Error::DuplicateDecision(decisions[0].subject))
    );
    let unknown = PackagePolicyDecision {
        subject: Subject::SourceReplacement([0; 32]),
        disposition: AcceptCandidateChange,
    };
    assert!(
        decisions
            .iter()
            .all(|choice| choice.subject != unknown.subject)
    );
    assert_eq!(
        resolve_package_policy_decisions(&changes, comparison, &[unknown]),
        Err(Error::UnknownSubject(unknown.subject))
    );
    fs::write(
        tree.path("sources/new/main.omg"),
        "// fresh candidate content\n",
    )
    .unwrap();
    let updated = compare(&tree, "updated", Some(&lock));
    assert_eq!(updated.source_replacements().len(), 2);
    for (old, new) in changes
        .source_replacements()
        .iter()
        .zip(updated.source_replacements())
    {
        assert_eq!(old.site(), new.site());
        assert_eq!(old.baseline(), new.baseline());
        assert_eq!(old.candidate(), new.candidate());
        assert_ne!(old.fingerprint(), new.fingerprint());
    }
    assert_eq!(
        resolve_package_policy_decisions(&updated, comparison, &decisions),
        Err(Error::WrongComparison)
    );
    assert_eq!(
        resolve_package_policy_decisions(&updated, updated.fingerprint().digest(), &decisions),
        Err(Error::UnknownSubject(decisions[0].subject))
    );
}
