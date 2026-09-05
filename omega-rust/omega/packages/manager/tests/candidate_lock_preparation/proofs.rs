use super::*;
use omega_package_evidence::ledger::OrdinaryPackageObligationStatus;

const ROOT: &str = "boundary machine trusted_zero() -> u64 ensures result == 0;\n";
const DEPENDENCY: &str = "builder.depend(Source::Path { location: \"../contract\" });\n";

fn contract_source(tree: &Tree, main: &str) {
    package(&tree.path("sources/contract"), "contract-surface", "");
    fs::write(tree.path("sources/contract/main.omg"), main).unwrap();
    source(tree, ROOT, DEPENDENCY);
}

#[test]
fn complete_policy_acceptance_cannot_discharge_a_dependency_proof_gap() {
    let tree = Tree::new();
    // Ordinary exit checking preserves the entry assumption, while the
    // polynomial entailment engine cannot represent min and stands down.
    contract_source(
        &tree,
        r#"pub machine unchecked_claim(a: u64, b: u64)
requires
    min(a, b) >= 1
ensures
    min(a, b) >= 1
{
}
"#,
    );
    let (sources, reviews) = candidate(&tree, "open-proof");
    let dependency = reviews
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "contract-surface")
        .unwrap();
    let dependency_key = dependency.key().clone();
    let [open] = dependency
        .obligation_results()
        .open_contract_entailment_obligations()
    else {
        panic!("the real dependency must retain exactly one proof gap");
    };
    assert_eq!(
        open.status(),
        OrdinaryPackageObligationStatus::OpenLaterDischarge
    );
    assert!(
        dependency
            .obligation_results()
            .contract_entailment_assumption_discharges()
            .is_empty()
    );
    assert!(
        reviews
            .review(sources.graph().root())
            .unwrap()
            .obligation_results()
            .open_contract_entailment_obligations()
            .is_empty()
    );

    let changes = compare(None, &sources, &reviews);
    let accepted = resolution(&changes, ACCEPT);
    assert!(
        !accepted.decisions().is_empty(),
        "the authored root claim needs real policy acceptance"
    );
    assert!(accepted.all_required_changes_accepted());
    let error = prepare_candidate_lock_target(
        None,
        &sources.for_exact_target(TARGET),
        reviews,
        &accepted,
        PrepareCandidateLockLimits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, PrepareCandidateLockError::OpenContractEntailment { package } if package == dependency_key)
    );
}

#[test]
fn independently_discharged_dependency_assumption_allows_preparation() {
    let tree = Tree::new();
    contract_source(
        &tree,
        r#"pub machine retain(value: u64) -> u64
requires
    value >= 1
ensures
    value >= 1
{
    let retained: u64 = value;
    retained
}
"#,
    );
    let (sources, reviews) = candidate(&tree, "discharged-proof");
    let dependency = reviews
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "contract-surface")
        .unwrap();
    let dependency_key = dependency.key().clone();
    assert!(
        dependency
            .obligation_results()
            .open_contract_entailment_obligations()
            .is_empty()
    );
    let [discharge] = dependency
        .obligation_results()
        .contract_entailment_assumption_discharges()
    else {
        panic!("the real dependency must carry its independently rechecked discharge");
    };
    assert_eq!(
        discharge.status(),
        OrdinaryPackageObligationStatus::Discharged
    );
    // The checked projection still discloses the original stand-down. Only
    // its exact compiler certificate closes the corresponding result.
    assert_eq!(
        dependency
            .projection()
            .contract_entailment_open_obligations()
            .len(),
        1
    );

    let changes = compare(None, &sources, &reviews);
    let accepted = resolution(&changes, ACCEPT);
    assert!(!accepted.decisions().is_empty());
    assert!(accepted.all_required_changes_accepted());
    let prepared = prepare_candidate_lock_target(
        None,
        &sources.for_exact_target(TARGET),
        reviews,
        &accepted,
        PrepareCandidateLockLimits::default(),
    )
    .expect("real proof discharge and exact accepted policy allow lock preparation");
    assert_eq!(prepared.target(), TARGET);
    assert!(
        prepared
            .source()
            .packages()
            .iter()
            .any(|package| package.key() == &dependency_key)
    );
    assert!(
        prepared
            .decisions()
            .decisions()
            .iter()
            .all(|decision| decision.disposition() == ACCEPT)
    );
    assert_eq!(
        prepared.decisions().decisions().len(),
        accepted.decisions().len()
    );
}
