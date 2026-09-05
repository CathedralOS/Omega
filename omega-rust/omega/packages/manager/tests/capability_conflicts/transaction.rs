use super::*;

#[path = "transaction/accepted_claims.rs"]
mod accepted_claims;
#[path = "transaction/candidate_conflicts.rs"]
mod candidate_conflicts;
#[path = "transaction/conflict_behavior.rs"]
mod conflict_behavior;
#[path = "transaction/fixture.rs"]
mod fixture;
#[path = "transaction/historical_policy.rs"]
mod historical_policy;
#[path = "transaction/lock_framing.rs"]
mod lock_framing;
#[path = "transaction/lock_membership_budget.rs"]
mod lock_membership_budget;
#[path = "transaction/package_lock.rs"]
mod package_lock;
#[path = "transaction/root_policy.rs"]
mod root_policy;

/// One exact compiler-row update remains bound to resolver custody through
/// conflict derivation, root policy, risk triage, and accepted-claim checks.
#[test]
fn exact_compiler_rows_become_candidate_bound_review_conflicts() {
    let scenario = fixture::ExactCompilerRowScenario::establish();
    let conflicts = candidate_conflicts::derive_and_assert(&scenario);

    historical_policy::assert_historical_policy(&scenario, &conflicts);
    root_policy::assert_persistence_and_recovery(&scenario, &conflicts);
    conflict_behavior::assert_comparison_limits_and_risk_classes(&scenario, &conflicts);
    accepted_claims::assert_candidate_binding(&scenario, &conflicts);
    package_lock::assert_complete_lock(&scenario, &conflicts);
}
