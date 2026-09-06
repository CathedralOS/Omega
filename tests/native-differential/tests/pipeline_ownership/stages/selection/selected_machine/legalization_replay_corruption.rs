//! Independent legalization replay rejection for foreign facts and detached leaf-operation custody.

use crate::tests::*;

#[test]
fn legalization_replay_rejects_foreign_proof_fact_and_leaf_operation_custody() {
    let staged = staged_exact_add_conditional(NativeTarget::linux_x64());
    let original = staged.legalized().plan();
    let validate = |plan| {
        validate_legalized_operations(
            staged.optimized_target().target_operations(),
            staged.optimized_target().optimized().plan(),
            staged.optimized_target().optimized().unit(),
            plan,
        )
    };

    let mut corrupted = original.clone();
    let false_fact = match corrupted.functions[0].conditional().when_false.value {
        legalized_operations::LegalizedLeafValue::ExactAdd { accepted_fact, .. } => accepted_fact,
        _ => panic!("exact-add fixture must retain its admitted fact"),
    };
    let legalized_operations::LegalizedLeafValue::ExactAdd { accepted_fact, .. } =
        &mut corrupted.functions[0].conditional_mut().when_true.value
    else {
        panic!("exact-add fixture must retain its admitted fact")
    };
    *accepted_fact = false_fact;
    assert_eq!(
        validate(corrupted),
        Err(LegalizationError::NonCanonicalLegalizedPlan)
    );

    let mut corrupted = original.clone();
    let legalized_operations::LegalizedLeafValue::ExactAdd { left, right, .. } =
        &mut corrupted.functions[0].conditional_mut().when_true.value
    else {
        panic!("exact-add fixture must retain its inputs")
    };
    std::mem::swap(&mut left.constant_operation, &mut right.constant_operation);
    assert_eq!(
        validate(corrupted),
        Err(LegalizationError::NonCanonicalLegalizedPlan)
    );
}
