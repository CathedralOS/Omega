use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use super::fixtures::{budget, source, validated_source};
use crate::*;

fn plan(source: &ValidatedLogicalSpillOperations) -> StackSlotColoringPlan {
    color_logical_spill_stack_slots(
        source,
        StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
        budget(),
    )
    .unwrap()
    .plan()
    .clone()
}

#[test]
fn validation_rejects_root_and_assignment_corruption() {
    let source = source();
    let mut wrong_root = plan(&source);
    wrong_root.logical_spill_operations = LogicalSpillOperationIdentity::from_bytes([99; 32]);
    assert_eq!(
        validate_stack_slot_coloring(&source, wrong_root),
        Err(StackSlotColoringError::RootMismatch)
    );

    let mut wrong_assignment = plan(&source);
    wrong_assignment.functions[0].assignments[0].spill_area_offset = 8;
    assert_eq!(
        validate_stack_slot_coloring(&source, wrong_assignment),
        Err(StackSlotColoringError::NonCanonicalAssignments { function: 0 })
    );
}

#[test]
fn validation_rejects_usage_and_budget_corruption() {
    let source = source();
    let mut wrong_usage = plan(&source);
    wrong_usage.usage = OptimizationWorkUsage {
        rule_evaluations: 1,
        candidates: 0,
        validation_steps: 1,
        commits: 0,
        iterations: 1,
    };
    assert_eq!(
        validate_stack_slot_coloring(&source, wrong_usage),
        Err(StackSlotColoringError::UsageMismatch)
    );

    let mut insufficient = plan(&source);
    insufficient.budget = OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap();
    assert!(matches!(
        validate_stack_slot_coloring(&source, insufficient),
        Err(StackSlotColoringError::BudgetExceeded { .. })
    ));
}

#[test]
fn malformed_logical_action_fails_closed() {
    let mut source = source();
    let storage = source.plan.functions[0].action.as_ref().unwrap().storage.id;
    source.plan.functions[0]
        .action
        .as_mut()
        .unwrap()
        .rewrites
        .clear();
    let source = validated_source(source.plan);
    assert_eq!(
        color_logical_spill_stack_slots(
            &source,
            StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1,
            budget(),
        ),
        Err(StackSlotColoringError::InvalidInterval {
            function: 0,
            storage,
        })
    );
}
