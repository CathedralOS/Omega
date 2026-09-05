//! Optimizer module role: stage group. Hosted publication matrix for exact machine rules.

mod artifacts;
mod cases;
mod realization;

#[test]
fn every_exact_machine_rule_publishes_deterministically_on_its_hosted_targets() {
    let cases = cases::hosted_cases();
    let represented = cases
        .iter()
        .map(|case| case.rule)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        represented,
        omega_post_allocation_machine_to_optimized_machine::ORDERED_POST_ALLOCATION_MACHINE_RULES
            .into_iter()
            .collect()
    );
    assert_eq!(cases.len(), 18);

    for case in cases {
        let first = artifacts::publish(realization::realize(case));
        let repeated = artifacts::publish(realization::realize(case));
        assert_eq!(first, repeated, "{:?} on {:?}", case.rule, case.target);
    }
}
