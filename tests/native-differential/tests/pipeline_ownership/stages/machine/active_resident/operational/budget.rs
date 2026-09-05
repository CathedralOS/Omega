//! Exact component usage and every representable first-over work boundary.

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_selected_instructions_to_register_homes::{
    PressureRematerializationError, RecoveryClassificationError, SpillChoiceError,
};

use crate::tests::{OptimizedActiveResidentRematerializationError, selected_lowering_budget};

use super::fixture::*;

fn exact_envelope(usages: [OptimizationWorkUsage; 3]) -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(
        usages
            .iter()
            .map(|usage| usage.rule_evaluations)
            .max()
            .unwrap(),
        usages.iter().map(|usage| usage.candidates).max().unwrap(),
        usages
            .iter()
            .map(|usage| usage.validation_steps)
            .max()
            .unwrap(),
        usages.iter().map(|usage| usage.commits).max().unwrap(),
        usages.iter().map(|usage| usage.iterations).max().unwrap(),
    )
    .unwrap()
}

fn first_over_boundary(
    exact: OptimizationWorkBudget,
    axis: usize,
) -> Option<OptimizationWorkBudget> {
    let mut fields = [
        exact.rule_evaluations(),
        exact.candidates(),
        exact.validation_steps(),
        exact.commits(),
        exact.iterations(),
    ];
    if fields[axis] == 1 {
        fields[axis] = 0;
        assert!(
            OptimizationWorkBudget::new(fields[0], fields[1], fields[2], fields[3], fields[4])
                .is_err()
        );
        return None;
    }
    fields[axis] -= 1;
    Some(
        OptimizationWorkBudget::new(fields[0], fields[1], fields[2], fields[3], fields[4]).unwrap(),
    )
}

fn budget_failure(
    error: OptimizedActiveResidentRematerializationError,
) -> (OptimizationWorkUsage, OptimizationWorkBudget) {
    match error {
        OptimizedActiveResidentRematerializationError::SpillChoice(
            SpillChoiceError::BudgetExceeded { required, budget },
        )
        | OptimizedActiveResidentRematerializationError::Classification(
            RecoveryClassificationError::BudgetExceeded { required, budget },
        )
        | OptimizedActiveResidentRematerializationError::Rematerialization(
            PressureRematerializationError::BudgetExceeded { required, budget },
        ) => (required, budget),
        other => panic!("expected an exact work-budget refusal, got {other:?}"),
    }
}

#[test]
fn active_resident_rule_pins_exact_work_and_every_representable_first_over_boundary() {
    for target in targets() {
        let reference = run(target, selected_lowering_budget()).unwrap();
        let usages = [
            reference.custody().choice_usage(),
            reference.custody().classification_usage(),
            reference.custody().rematerialization_usage(),
        ];
        assert_eq!(
            usages,
            [
                OptimizationWorkUsage {
                    rule_evaluations: 4,
                    candidates: 3,
                    validation_steps: 9,
                    commits: 1,
                    iterations: 1,
                },
                OptimizationWorkUsage {
                    rule_evaluations: 1,
                    candidates: 1,
                    validation_steps: 27,
                    commits: 1,
                    iterations: 1,
                },
                OptimizationWorkUsage {
                    rule_evaluations: 1,
                    candidates: 1,
                    validation_steps: 21,
                    commits: 1,
                    iterations: 1,
                },
            ]
        );
        let exact = exact_envelope(usages);
        let exact_run = run(target, exact).unwrap();
        assert_eq!(exact_run.custody().budget(), exact);
        assert!(usages.iter().all(|usage| usage.within(exact)));

        for axis in 0..5 {
            let Some(insufficient) = first_over_boundary(exact, axis) else {
                continue;
            };
            let first_error = run(target, insufficient).unwrap_err();
            let repeated_error = run(target, insufficient).unwrap_err();
            assert_eq!(first_error, repeated_error);
            let (required, rejected_budget) = budget_failure(first_error);
            assert_eq!(rejected_budget, insufficient);
            assert!(!required.within(insufficient));
            assert!(required.within(exact));
        }
    }
}
