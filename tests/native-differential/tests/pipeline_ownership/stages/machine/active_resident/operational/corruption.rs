//! Direct action-plan and enclosing custody corruption rejection.

use selected_instructions_to_register_homes::{
    PressureRematerializationError, validate_pressure_rematerialization,
};
use semantic_vocabulary::IntegerValue;

use crate::tests::{
    NativeTarget, OptimizedActiveResidentRematerializationError, selected_lowering_budget,
    validate_optimized_active_resident_rematerialization,
};

use super::fixture::*;

#[test]
fn active_resident_rule_rejects_action_and_enclosing_custody_corruption() {
    let target = NativeTarget::linux_x64();
    let staged = run(target, selected_lowering_budget()).unwrap();
    let source = staged.source();
    let environment = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let selected = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .selected();
    let mut corrupted_plan = staged.rematerialization().plan().clone();
    corrupted_plan.functions[0].action.as_mut().unwrap().value = IntegerValue::Unsigned(4);
    assert!(matches!(
        validate_pressure_rematerialization(
            selected,
            source.live_range_stage().ranges(),
            source.legality(),
            staged.choices(),
            staged.classifications(),
            source.allocator_availability(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            corrupted_plan,
        ),
        Err(PressureRematerializationError::DecisionMismatch { .. })
    ));

    let mut corrupted_custody = staged;
    crate::corrupt_active_resident_rematerialization_custody_for_test(&mut corrupted_custody);
    assert_eq!(
        validate_optimized_active_resident_rematerialization(&corrupted_custody),
        Err(OptimizedActiveResidentRematerializationError::ReceiptMismatch)
    );
}
