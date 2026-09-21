//! Representation-specialization publication custody.
use super::{
    Optimization, OptimizationSelections, representation_specialization_field_value_verified, run,
};
use crate::publish_optimization_run;
use optimization_core::{OptimizationCandidateVerdict, OptimizationRuleIdentity};

/// The representation pass's second rule publishes and replays through the
/// same public selection: a source-produced record establishment proves two
/// scalar field reads, the field-value rule commits once, and the published
/// plan carries the applied decision with its validator custody.
#[test]
fn field_value_rule_commits_and_replays_under_the_same_selection() {
    let selections =
        OptimizationSelections::new([Optimization::RepresentationSpecialization]).unwrap();
    let optimized = publish_optimization_run(run(
        representation_specialization_field_value_verified(),
        selections,
    ))
    .unwrap();
    let field_rule = OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.field-value-specialization.v1",
    );
    assert_eq!(optimized.commits().len(), 1);
    assert!(
        optimized
            .commits()
            .iter()
            .any(|commit| commit.rule == field_rule),
        "the field-value fixture must commit through the field-value rule"
    );
    assert_eq!(
        optimized
            .pass_manifests()
            .iter()
            .flat_map(|manifest| manifest.decisions())
            .filter(|decision| decision.verdict() == OptimizationCandidateVerdict::Applied)
            .count(),
        optimized.commits().len()
    );
}
