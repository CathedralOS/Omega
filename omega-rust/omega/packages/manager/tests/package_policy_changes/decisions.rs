use super::*;
#[path = "decisions/context.rs"]
mod context;

use omega_package_manager::review::{
    PackagePolicyChangeSet, PackagePolicyDecision, PackagePolicyDecisionError as Error,
    PackagePolicyDecisionSubject as Subject, ReviewOnlyRootPolicyDisposition::*,
    resolve_package_policy_decisions,
};

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
    let (closure, reviews) = candidate(tree, "assumptions");
    compare(None, &closure, &reviews)
}

fn accepting(changes: &PackagePolicyChangeSet) -> Vec<PackagePolicyDecision> {
    let mut decisions = Vec::new();
    if changes.root_role_change().is_some() {
        decisions.push(PackagePolicyDecision {
            subject: Subject::RootRole,
            disposition: AcceptCandidateChange,
        });
    }
    for row in changes.packages().iter().flat_map(|package| package.rows()) {
        if row.requires_decision() {
            decisions.push(PackagePolicyDecision {
                subject: Subject::Row(row.fingerprint().digest()),
                disposition: AcceptCandidateChange,
            });
        }
    }
    decisions.sort_by_key(|decision| decision.subject);
    decisions
}

#[test]
fn initial_pure_policy_resolves_without_decisions_and_rejects_extra_choices() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (closure, reviews) = candidate(&tree, "pure");
    let changes = compare(None, &closure, &reviews);
    assert!(!changes.requires_decision());
    let comparison = changes.fingerprint().digest();
    let resolution = resolve_package_policy_decisions(&changes, comparison, &[]).unwrap();
    assert_eq!(resolution.comparison(), changes.fingerprint());
    assert!(resolution.decisions().is_empty());
    assert!(resolution.all_required_changes_accepted());
    let row = changes.packages()[0]
        .rows()
        .iter()
        .find(|row| row.kind() == PackagePolicyRowKind::PublicConst && !row.requires_decision())
        .unwrap();
    let extra = PackagePolicyDecision {
        subject: Subject::Row(row.fingerprint().digest()),
        disposition: AcceptCandidateChange,
    };
    assert_eq!(
        resolve_package_policy_decisions(&changes, comparison, &[extra]),
        Err(Error::TooManyDecisions)
    );
}

#[test]
fn initial_assumptions_require_every_choice_and_nonblocking_rows_reject() {
    let tree = Tree::new();
    let changes = initial_assumptions(&tree);
    let comparison = changes.fingerprint().digest();
    let decisions = accepting(&changes);
    assert!(decisions.len() >= 2);
    assert_eq!(
        resolve_package_policy_decisions(&changes, comparison, &[]),
        Err(Error::MissingDecision(decisions[0].subject))
    );
    for missing in &decisions {
        let remaining: Vec<_> = decisions
            .iter()
            .copied()
            .filter(|decision| decision.subject != missing.subject)
            .collect();
        assert_eq!(
            resolve_package_policy_decisions(&changes, comparison, &remaining),
            Err(Error::MissingDecision(missing.subject))
        );
    }
    let row = changes.packages()[0]
        .rows()
        .iter()
        .find(|row| row.kind() == PackagePolicyRowKind::PublicConst && !row.requires_decision())
        .unwrap();
    for disposition in [AcceptCandidateChange, RejectCandidateChange] {
        let nonblocking = PackagePolicyDecision {
            subject: Subject::Row(row.fingerprint().digest()),
            disposition,
        };
        assert_eq!(
            resolve_package_policy_decisions(&changes, comparison, &[nonblocking]),
            Err(Error::NonBlockingChange(nonblocking.subject))
        );
    }
}

#[test]
fn mixed_choices_are_retained_in_canonical_order_and_duplicates_reject() {
    let tree = Tree::new();
    let changes = initial_assumptions(&tree);
    let comparison = changes.fingerprint().digest();
    let decisions = accepting(&changes);
    assert!(decisions.len() >= 2);
    let accepted = resolve_package_policy_decisions(&changes, comparison, &decisions).unwrap();
    assert!(accepted.all_required_changes_accepted());
    assert_eq!(accepted.comparison(), changes.fingerprint());
    assert_eq!(accepted.decisions(), decisions);
    for rejected in 0..decisions.len() {
        let mut mixed = decisions.clone();
        mixed[rejected].disposition = RejectCandidateChange;
        let expected = mixed.clone();
        mixed.reverse();
        let resolution = resolve_package_policy_decisions(&changes, comparison, &mixed).unwrap();
        assert!(!resolution.all_required_changes_accepted());
        assert_eq!(resolution.decisions(), expected);
    }
    let mut repeated = decisions.clone();
    repeated[1] = repeated[0];
    repeated[1].disposition = RejectCandidateChange;
    assert_eq!(
        resolve_package_policy_decisions(&changes, comparison, &repeated),
        Err(Error::DuplicateDecision(repeated[0].subject))
    );
}
