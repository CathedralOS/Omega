use crate::tests::*;
use omega_regalloc::LiteralFoldError;

use super::fixture::*;

#[test]
fn exact_subtract_rule_rejects_action_corruption() {
    let run = run(
        NativeTarget::linux_x64(),
        "rax",
        true,
        selected_lowering_budget(),
    )
    .unwrap();
    let source = run.source_legality_stage();
    let selected = source.live_range_stage().liveness_stage().selected_stage();
    let environment = selected.register_environment();
    let first = &run.steps()[0];
    let mut corrupted = first.fold().plan().clone();
    corrupted.functions[0].action.as_mut().unwrap().immediate += 1;

    assert_eq!(
        validate_literal_fold(
            selected.selected(),
            source.live_range_stage().ranges(),
            source.legality(),
            first.choices(),
            first.recovery(),
            source.allocator_availability(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            environment.allocation_constraint_keys(),
            corrupted,
        ),
        Err(LiteralFoldError::DecisionMismatch { function: 0 })
    );
}
