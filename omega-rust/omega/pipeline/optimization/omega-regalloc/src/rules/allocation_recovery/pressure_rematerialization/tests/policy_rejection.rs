use omega_optimization_core::OptimizationWorkBudget;
use omega_register_model::RegisterViewId;
use omega_selected_instructions::VirtualRegisterId;

use super::super::compute::{build_functions, ensure_budget, required_usage};
use super::super::{
    PressureRecoveryClassification, PressureRematerializationError,
    PressureRematerializationPolicy, RecoveryClassification,
};
use super::fixtures::{fixture, multiple_future_fixture, same_instruction_multiple_future_fixture};

#[test]
fn multiple_use_policy_rejects_noncanonical_or_single_rewrite_evidence() {
    let (single_selected, single_ranges, single_recovery, row) = fixture();
    assert!(matches!(
        build_functions(
            &single_selected,
            &single_ranges,
            &single_recovery,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        ),
        Err(PressureRematerializationError::FutureUseMismatch { function: 0 })
    ));

    let (selected, ranges, mut recovery, row) = multiple_future_fixture();
    {
        let Some(PressureRecoveryClassification {
            classification:
                RecoveryClassification::ImmediateU64RematerializationCandidate { future_uses, .. },
            ..
        }) = recovery.functions[0].classification.as_mut()
        else {
            unreachable!()
        };
        future_uses.swap(0, 1);
    }
    assert!(matches!(
        build_functions(
            &selected,
            &ranges,
            &recovery,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        ),
        Err(PressureRematerializationError::FutureUseMismatch { function: 0 })
    ));
    {
        let Some(PressureRecoveryClassification {
            classification:
                RecoveryClassification::ImmediateU64RematerializationCandidate { future_uses, .. },
            ..
        }) = recovery.functions[0].classification.as_mut()
        else {
            unreachable!()
        };
        future_uses.swap(0, 1);
        future_uses[1] = future_uses[0];
    }
    assert!(matches!(
        build_functions(
            &selected,
            &ranges,
            &recovery,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        ),
        Err(PressureRematerializationError::FutureUseMismatch { function: 0 })
    ));

    let (selected, ranges, recovery, row) = same_instruction_multiple_future_fixture();
    let usage = required_usage(&selected, 1, 2).unwrap();
    assert_eq!(usage.validation_steps, 10);
    let insufficient = OptimizationWorkBudget::new(
        usage.rule_evaluations,
        usage.candidates,
        usage.validation_steps - 1,
        usage.commits,
        usage.iterations,
    )
    .unwrap();
    assert_eq!(
        ensure_budget(usage, insufficient),
        Err(PressureRematerializationError::BudgetExceeded {
            required: usage,
            budget: insufficient,
        })
    );
    let (functions, transformed) = build_functions(
        &selected,
        &ranges,
        &recovery,
        &row,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
    )
    .unwrap();
    assert_eq!(functions[0].action.as_ref().unwrap().rewrites.len(), 2);
    assert_eq!(
        transformed.functions[0].blocks[0].instructions[4]
            .operands
            .iter()
            .map(|operand| operand.virtual_register)
            .collect::<Vec<_>>(),
        vec![VirtualRegisterId(3), VirtualRegisterId(3)]
    );

    let (mut selected, ranges, recovery, row) = multiple_future_fixture();
    selected.functions[0].blocks[0].instructions[3].operands[0].fixed_view =
        Some(RegisterViewId(0));
    assert!(matches!(
        build_functions(
            &selected,
            &ranges,
            &recovery,
            &row,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        ),
        Err(PressureRematerializationError::FutureUseMismatch { function: 0 })
    ));
}
