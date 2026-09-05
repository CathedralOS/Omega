use super::*;
use package_manager::review::{
    PackagePolicyChangeSet, PackagePolicyDecision, PackagePolicyDecisionError,
    PackagePolicyDecisionSubject as Subject, PackagePolicyReviewError as Error,
    ReviewOnlyRootPolicyDisposition::*, recover_package_policy_review,
    render_package_policy_review, resolve_package_policy_decisions,
};

const MAXIMUM_BYTES: usize = 4 * 1024 * 1024;
const ASSUMPTIONS: &str = concat!(
    "pub const VALUE: u64 = 7;\n",
    "boundary machine trusted_zero() -> u64 ensures result == 0;\n",
    "boundary machine trusted_one() -> u64 ensures result == 1;\n",
);

fn compare(
    accepted: Option<&PackageLockTarget>,
    closure: &ResolvedPackageSourceClosure,
    reviews: &CompilerIssuedPackageReviewSet,
) -> PackagePolicyChangeSet {
    compare_package_policy_changes(
        accepted,
        reviews,
        &closure.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .unwrap()
}

fn initial_assumptions(tree: &Tree) -> PackagePolicyChangeSet {
    source(tree, ASSUMPTIONS, "");
    let (closure, reviews) = candidate(tree, "document-assumptions");
    compare(None, &closure, &reviews)
}

fn hex(digest: [u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn choice_line(subject: Subject, choice: &str) -> String {
    match subject {
        Subject::RootRole => format!("decision root-role {choice}\n"),
        Subject::SourceReplacement(digest) => {
            format!("decision source-replacement {} {choice}\n", hex(digest))
        }
        Subject::Row(digest) => format!("decision row {} {choice}\n", hex(digest)),
    }
}

fn accepting(changes: &PackagePolicyChangeSet) -> Vec<PackagePolicyDecision> {
    let mut subjects = Vec::new();
    if changes.root_role_change().is_some() {
        subjects.push(Subject::RootRole);
    }
    subjects.extend(
        changes
            .source_replacements()
            .iter()
            .map(|replacement| Subject::SourceReplacement(replacement.fingerprint().digest())),
    );
    subjects.extend(
        changes
            .packages()
            .iter()
            .flat_map(|package| package.rows())
            .filter(|row| row.requires_decision())
            .map(|row| Subject::Row(row.fingerprint().digest())),
    );
    subjects.sort();
    subjects
        .into_iter()
        .map(|subject| PackagePolicyDecision {
            subject,
            disposition: AcceptCandidateChange,
        })
        .collect()
}

fn accept_document(changes: &PackagePolicyChangeSet, template: &str) -> String {
    accepting(changes)
        .iter()
        .fold(template.to_owned(), |text, decision| {
            text.replace(
                &choice_line(decision.subject, "pending"),
                &choice_line(decision.subject, "accept"),
            )
        })
}

fn assert_choices_round_trip(changes: &PackagePolicyChangeSet) {
    let template = render_package_policy_review(changes, MAXIMUM_BYTES).unwrap();
    assert!(template.starts_with(&format!(
        "omega-package-review 1\ncomparison {}\n",
        hex(changes.fingerprint().digest())
    )));
    assert!(template.ends_with("end-review\n"));
    assert!(!template.contains('\r'));
    assert_eq!(
        render_package_policy_review(changes, MAXIMUM_BYTES).unwrap(),
        template
    );
    let decisions = accepting(changes);
    let mut actual: Vec<_> = template
        .split_inclusive('\n')
        .filter(|line| line.starts_with("decision "))
        .collect();
    actual.sort();
    let mut expected: Vec<_> = decisions
        .iter()
        .map(|decision| choice_line(decision.subject, "pending"))
        .collect();
    expected.sort();
    assert_eq!(actual, expected);
    let accepted = accept_document(changes, &template);
    let resolution = recover_package_policy_review(changes, &accepted, MAXIMUM_BYTES).unwrap();
    assert_eq!(resolution.comparison(), changes.fingerprint());
    assert_eq!(resolution.decisions(), decisions);
    assert!(resolution.all_required_changes_accepted());
    assert_eq!(
        resolution,
        resolve_package_policy_decisions(changes, changes.fingerprint().digest(), &decisions)
            .unwrap()
    );
    for (index, decision) in decisions.iter().enumerate() {
        // Leave each subject pending in turn so another unresolved subject
        // cannot hide a missing check for this one.
        let pending = accepted.replace(
            &choice_line(decision.subject, "accept"),
            &choice_line(decision.subject, "pending"),
        );
        assert_eq!(
            recover_package_policy_review(changes, &pending, MAXIMUM_BYTES),
            Err(Error::UnresolvedDecision(decision.subject))
        );
        let rejected = accepted.replace(
            &choice_line(decision.subject, "accept"),
            &choice_line(decision.subject, "reject"),
        );
        let resolution = recover_package_policy_review(changes, &rejected, MAXIMUM_BYTES).unwrap();
        let mut expected = decisions.clone();
        expected[index].disposition = RejectCandidateChange;
        assert_eq!(resolution.decisions(), expected);
        assert!(!resolution.all_required_changes_accepted());
    }
}

#[test]
fn pure_initial_policy_has_readable_findings_and_no_choices() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (closure, reviews) = candidate(&tree, "document-pure");
    let changes = compare(None, &closure, &reviews);
    assert!(!changes.requires_decision());
    let text = render_package_policy_review(&changes, MAXIMUM_BYTES).unwrap();
    assert!(text.lines().any(|line| line == "baseline none"));
    assert!(text.contains("policy-fixture"));
    assert!(text.contains("VALUE"));
    assert!(!text.lines().any(|line| line.starts_with("decision ")));
    assert_choices_round_trip(&changes);
}

#[test]
fn initial_assumptions_require_explicit_choices_and_preserve_rejection() {
    let tree = Tree::new();
    let changes = initial_assumptions(&tree);
    assert!(accepting(&changes).len() >= 2);
    assert_choices_round_trip(&changes);
}

#[test]
fn advisory_representation_findings_never_receive_choices() {
    let tree = Tree::new();
    source(
        &tree,
        concat!(
            "use omega::language::core::representation;\n",
            "pub boundary data Token;\n",
            "pub data Carrier { value: u64; }\n",
            "pub TokenRepresentation: Carrier satisfies OpaqueRepresentation<Token>;\n",
        ),
        " builder.select_representation<Token, TokenRepresentation>();\n",
    );
    let (closure, reviews) = candidate(&tree, "document-advisory");
    let changes = compare(None, &closure, &reviews);
    let row = changes.packages()[0]
        .rows()
        .iter()
        .find(|row| row.kind() == PackagePolicyRowKind::RepresentationSelection)
        .unwrap();
    assert!(row.audit_recommended());
    assert!(!row.requires_decision());
    let template = render_package_policy_review(&changes, MAXIMUM_BYTES).unwrap();
    assert!(template.contains("TokenRepresentation"));
    assert_choices_round_trip(&changes);
    let extra = format!(
        "{}{}",
        accept_document(&changes, &template),
        choice_line(Subject::Row(row.fingerprint().digest()), "accept")
    );
    assert!(recover_package_policy_review(&changes, &extra, MAXIMUM_BYTES).is_err());
}

#[test]
fn only_exact_accept_and_reject_tokens_complete_a_choice() {
    let tree = Tree::new();
    let changes = initial_assumptions(&tree);
    let template = render_package_policy_review(&changes, MAXIMUM_BYTES).unwrap();
    let accepted = accept_document(&changes, &template);
    let subject = accepting(&changes)[0].subject;
    for token in [
        "",
        "accepted",
        "approve",
        "ACCEPT",
        "Reject",
        "accept reject",
    ] {
        let edited = accepted.replace(
            &choice_line(subject, "accept"),
            &choice_line(subject, token),
        );
        assert_eq!(
            recover_package_policy_review(&changes, &edited, MAXIMUM_BYTES),
            Err(Error::InvalidDecision),
            "token {token:?}"
        );
    }
}

#[test]
fn edited_missing_duplicate_foreign_and_advisory_choice_lines_reject() {
    let tree = Tree::new();
    let changes = initial_assumptions(&tree);
    let template = render_package_policy_review(&changes, MAXIMUM_BYTES).unwrap();
    let accepted = accept_document(&changes, &template);
    let decisions = accepting(&changes);
    let first = choice_line(decisions[0].subject, "accept");
    let second = choice_line(decisions[1].subject, "accept");
    let advisory = changes.packages()[0]
        .rows()
        .iter()
        .find(|row| row.kind() == PackagePolicyRowKind::PublicConst && !row.requires_decision())
        .unwrap();
    let foreign_tree = Tree::new();
    let foreign = initial_assumptions(&foreign_tree);
    let foreign_subject = accepting(&foreign)[0].subject;
    assert!(
        decisions
            .iter()
            .all(|decision| decision.subject != foreign_subject)
    );
    for (label, edited) in [
        ("missing", accepted.replace(&first, "")),
        ("duplicate", accepted.replace(&second, &first)),
        (
            "extra",
            accepted.replace(&first, &format!("{first}{first}")),
        ),
        (
            "foreign",
            accepted.replace(&first, &choice_line(foreign_subject, "accept")),
        ),
        (
            "advisory",
            accepted.replace(
                &first,
                &choice_line(Subject::Row(advisory.fingerprint().digest()), "accept"),
            ),
        ),
        (
            "subject kind",
            accepted.replace(&first, &first.replace(" row ", " source-replacement ")),
        ),
        (
            "reordered",
            accepted
                .replace(&first, "SWAP\n")
                .replace(&second, &first)
                .replace("SWAP\n", &second),
        ),
        (
            "spacing",
            accepted.replace(&first, &first.replace("decision row ", "decision  row ")),
        ),
    ] {
        assert_eq!(
            recover_package_policy_review(&changes, &edited, MAXIMUM_BYTES),
            Err(Error::ChangedFindings),
            "{label}"
        );
    }
}

#[test]
fn findings_and_exact_line_framing_are_immutable_even_with_pending_choices() {
    let tree = Tree::new();
    let changes = initial_assumptions(&tree);
    let template = render_package_policy_review(&changes, MAXIMUM_BYTES).unwrap();
    let finding = template
        .split_inclusive('\n')
        .find(|line| line.starts_with("+ ") && line.contains("trusted_zero"))
        .expect("readable canonical assumption row");
    for text in [&template, &accept_document(&changes, &template)] {
        for edited in [
            text.replace("omega-package-review 1\n", "omega-package-review 2\n"),
            text.replace(finding, &finding.replace("trusted_zero", "trusted_other")),
            text.replace(finding, ""),
            text.replace(finding, &format!("{finding}{finding}")),
            text.replace('\n', "\r\n"),
            format!("{text}\n"),
            text.strip_suffix('\n').unwrap().to_owned(),
        ] {
            assert_eq!(
                recover_package_policy_review(&changes, &edited, MAXIMUM_BYTES),
                Err(Error::ChangedFindings)
            );
        }
    }
}

#[test]
fn byte_limits_bound_both_input_and_regenerated_template() {
    let tree = Tree::new();
    let changes = initial_assumptions(&tree);
    let template = render_package_policy_review(&changes, MAXIMUM_BYTES).unwrap();
    assert_eq!(
        render_package_policy_review(&changes, template.len()).unwrap(),
        template
    );
    for maximum_bytes in [0, template.len() - 1] {
        assert_eq!(
            render_package_policy_review(&changes, maximum_bytes),
            Err(Error::ByteLimit)
        );
    }
    let accepted = accept_document(&changes, &template);
    assert!(accepted.len() < template.len());
    assert!(recover_package_policy_review(&changes, &accepted, template.len()).is_ok());
    // Input fits, but recovery must also fit the original pending template.
    assert_eq!(
        recover_package_policy_review(&changes, &accepted, accepted.len()),
        Err(Error::ByteLimit)
    );
    for maximum_bytes in [0, accepted.len() - 1] {
        assert_eq!(
            recover_package_policy_review(&changes, &accepted, maximum_bytes),
            Err(Error::ByteLimit)
        );
    }
    let oversized = format!("{accepted}{}", " ".repeat(template.len()));
    assert_eq!(
        recover_package_policy_review(&changes, &oversized, template.len()),
        Err(Error::ByteLimit)
    );
}

#[test]
fn source_only_edits_invalidate_saved_comparisons_even_when_policy_is_identical() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (baseline_sources, baseline_reviews) = candidate(&tree, "document-baseline");
    let lock = lock_from_reviews(&baseline_sources, &baseline_reviews);
    source(&tree, ASSUMPTIONS, "");
    let (original_sources, original_reviews) = candidate(&tree, "document-original");
    let original = compare(lock.target(TARGET), &original_sources, &original_reviews);
    let template = render_package_policy_review(&original, MAXIMUM_BYTES).unwrap();
    let saved = accept_document(&original, &template);
    source(
        &tree,
        &format!("// source-only revision\n{ASSUMPTIONS}"),
        "",
    );
    let (updated_sources, updated_reviews) = candidate(&tree, "document-updated");
    let root = original_sources.graph().root();
    assert_eq!(root, updated_sources.graph().root());
    assert_eq!(
        original_reviews.review(root).unwrap().policy(),
        updated_reviews.review(root).unwrap().policy()
    );
    let updated = compare(lock.target(TARGET), &updated_sources, &updated_reviews);
    assert_ne!(original.fingerprint(), updated.fingerprint());
    for text in [&template, &saved] {
        assert_eq!(
            recover_package_policy_review(&updated, text, MAXIMUM_BYTES),
            Err(Error::Decisions(
                PackagePolicyDecisionError::WrongComparison
            ))
        );
    }
    let current = render_package_policy_review(&updated, MAXIMUM_BYTES).unwrap();
    let wrong_comparison = current.replace(
        &format!("comparison {}\n", hex(updated.fingerprint().digest())),
        &format!("comparison {}\n", hex(original.fingerprint().digest())),
    );
    assert_eq!(
        recover_package_policy_review(&updated, &wrong_comparison, MAXIMUM_BYTES),
        Err(Error::Decisions(
            PackagePolicyDecisionError::WrongComparison
        ))
    );
    assert_choices_round_trip(&updated);
}

#[test]
fn added_changed_and_removed_rows_render_exact_readable_canonical_lines() {
    let tree = Tree::new();
    source(
        &tree,
        "pub const CHANGED: u64 = 1;\npub const REMOVED: u64 = 1;\n",
        "",
    );
    let (baseline_sources, baseline_reviews) = candidate(&tree, "document-old-rows");
    let lock = lock_from_reviews(&baseline_sources, &baseline_reviews);
    source(
        &tree,
        "pub const CHANGED: u64 = 2;\npub const ADDED: u64 = 1;\n",
        "",
    );
    let (closure, reviews) = candidate(&tree, "document-new-rows");
    let changes = compare(lock.target(TARGET), &closure, &reviews);
    let text = render_package_policy_review(&changes, MAXIMUM_BYTES).unwrap();
    let rows = changes.packages()[0].rows();
    for kind in [
        PackagePolicyChangeKind::Added,
        PackagePolicyChangeKind::Changed,
        PackagePolicyChangeKind::Removed,
    ] {
        assert!(rows.iter().any(|row| row.change() == kind));
    }
    for row in rows {
        for (prefix, value) in [("- ", row.baseline()), ("+ ", row.candidate())] {
            if let Some(value) = value {
                let expected: String = value
                    .canonical_text()
                    .lines()
                    .map(|line| format!("{prefix}{line}\n"))
                    .collect();
                assert!(
                    text.contains(&expected),
                    "missing canonical row:\n{expected}"
                );
            }
        }
    }
    assert_choices_round_trip(&changes);
}

#[test]
fn root_role_choices_round_trip_in_both_directions_alongside_rows() {
    let tree = Tree::new();
    let main = "data Main { }\nmachine Main::main(&mut self) { }\n";
    source(&tree, &format!("{main}pub const VALUE: u64 = 7;\n"), "");
    let (package_sources, package_reviews) = candidate(&tree, "document-package");
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
    let (application_sources, application_reviews) = candidate(&tree, "document-application");
    let application_lock = lock_from_reviews(&application_sources, &application_reviews);
    for changes in [
        compare(
            package_lock.target(TARGET),
            &application_sources,
            &application_reviews,
        ),
        compare(
            application_lock.target(TARGET),
            &package_sources,
            &package_reviews,
        ),
    ] {
        assert!(changes.root_role_change().is_some());
        assert!(accepting(&changes).len() >= 2);
        assert_choices_round_trip(&changes);
    }
}

#[test]
fn source_replacement_choices_round_trip_for_exact_dependency_aliases() {
    let tree = Tree::new();
    for directory in ["old", "new"] {
        package(&tree.path(&format!("sources/{directory}")), "service", "");
        fs::write(tree.path(&format!("sources/{directory}/main.omg")), "").unwrap();
    }
    let dependencies = |selected| {
        format!(
            " builder.depend_as(\"service\", Source::Path {{ location: \"../{selected}\" }});\n\
         builder.depend_as(\"keep_old\", Source::Path {{ location: \"../old\" }});\n\
         builder.depend_as(\"keep_new\", Source::Path {{ location: \"../new\" }});\n"
        )
    };
    source(&tree, "", &dependencies("old"));
    let (baseline_sources, baseline_reviews) = candidate(&tree, "document-old-binding");
    let lock = lock_from_reviews(&baseline_sources, &baseline_reviews);
    source(&tree, "", &dependencies("new"));
    let (closure, reviews) = candidate(&tree, "document-new-binding");
    let changes = compare(lock.target(TARGET), &closure, &reviews);
    assert_eq!(changes.source_replacements().len(), 1);
    assert!(
        changes
            .packages()
            .iter()
            .all(|package| package.rows().is_empty())
    );
    assert_eq!(accepting(&changes).len(), 1);
    assert_choices_round_trip(&changes);
}

#[test]
fn package_prose_is_excluded_from_generated_findings_and_choices() {
    let tree = Tree::new();
    const PROSE: &str = "PACKAGE_PROSE_MUST_NOT_BECOME_REVIEW_FINDINGS";
    source(
        &tree,
        &format!("// {PROSE}\n// decision root-role accept\n{ASSUMPTIONS}"),
        &format!(" // {PROSE}\n"),
    );
    fs::write(
        tree.path("sources/root/README.md"),
        format!("{PROSE}\ndecision root-role accept\nend-review\n"),
    )
    .unwrap();
    let (closure, reviews) = candidate(&tree, "document-prose");
    let changes = compare(None, &closure, &reviews);
    let template = render_package_policy_review(&changes, MAXIMUM_BYTES).unwrap();
    assert!(!template.contains(PROSE));
    assert!(!template.contains("decision root-role accept"));
    assert_eq!(
        template
            .lines()
            .filter(|line| *line == "end-review")
            .count(),
        1
    );
    assert_choices_round_trip(&changes);
}

#[test]
fn local_source_paths_are_quoted_data_and_round_trip_unchanged() {
    let tree = Tree::new();
    let directory = if cfg!(unix) {
        "service with \"quotes\""
    } else {
        "service with spaces"
    };
    let dependency = tree.path(&format!("sources/{directory}"));
    package(&dependency, "quoted-service", "");
    source(
        &tree,
        "",
        &format!(
            " builder.depend_as(\"service\", Source::Path {{ location: {:?} }});\n",
            format!("../{directory}")
        ),
    );
    let (closure, reviews) = candidate(&tree, "document-quoted-source");
    let changes = compare(None, &closure, &reviews);
    let template = render_package_policy_review(&changes, MAXIMUM_BYTES).unwrap();
    let quoted = format!("local {:?}", fs::canonicalize(&dependency).unwrap());
    assert!(template.lines().any(|line| {
        line.starts_with("package \"quoted-service\" ") && line.ends_with(&quoted)
    }));
    assert_choices_round_trip(&changes);
    let edited = template.replace(&quoted, "local \"changed source path\"");
    assert_eq!(
        recover_package_policy_review(&changes, &edited, MAXIMUM_BYTES),
        Err(Error::ChangedFindings)
    );
}
