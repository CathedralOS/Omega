//! Deterministic assignment with retained optimizer ledger and manifest custody.

use crate::tests::*;

#[test]
fn staged_assignment_is_deterministic_and_retains_optimizer_custody() {
    let (semantic, proof) = artifact();
    let selections = OptimizationSelections::new([
        Optimization::SparseConditionalConstantPropagation,
        Optimization::CopyPropagation,
    ])
    .unwrap();
    let stage = || {
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(selections.clone()),
        )
        .unwrap();
        let target =
            lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64()).unwrap();
        stage_optimized_assignment(target).unwrap()
    };
    let first = stage();
    let second = stage();

    assert_eq!(first.assigned(), second.assigned());
    assert_eq!(first.custody(), second.custody());
    assert_eq!(
        first.optimized_target().optimized().transformation_ledger(),
        second
            .optimized_target()
            .optimized()
            .transformation_ledger()
    );
    assert_eq!(
        first.optimized_target().optimized().pass_manifests(),
        second.optimized_target().optimized().pass_manifests()
    );
}
