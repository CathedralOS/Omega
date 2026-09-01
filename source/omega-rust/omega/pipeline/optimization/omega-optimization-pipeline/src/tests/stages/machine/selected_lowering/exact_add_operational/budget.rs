use crate::tests::*;

use super::fixture::*;

fn budget_fields(budget: OptimizationWorkBudget) -> [u64; 5] {
    [
        budget.rule_evaluations(),
        budget.candidates(),
        budget.validation_steps(),
        budget.commits(),
        budget.iterations(),
    ]
}

fn first_over_boundary(
    exact: OptimizationWorkBudget,
    axis: usize,
) -> Option<OptimizationWorkBudget> {
    let mut fields = budget_fields(exact);
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

#[test]
fn exact_add_rule_pins_exact_aggregate_work_and_deterministic_first_over_boundaries() {
    for (target, sole_view_name) in targets() {
        let reference = run(target, sole_view_name, true, selected_lowering_budget()).unwrap();
        let usage = reference.custody().usage();
        assert_eq!(
            usage,
            OptimizationWorkUsage {
                rule_evaluations: 19,
                candidates: 8,
                validation_steps: 123,
                commits: 6,
                iterations: 9,
            }
        );
        let exact = OptimizationWorkBudget::new(
            usage.rule_evaluations,
            usage.candidates,
            usage.validation_steps,
            usage.commits,
            usage.iterations,
        )
        .unwrap();
        let exact_run = run(target, sole_view_name, true, exact).unwrap();
        assert_eq!(exact_run.custody().usage(), usage);
        assert_eq!(exact_run.custody().budget(), exact);

        for axis in 0..5 {
            let Some(insufficient) = first_over_boundary(exact, axis) else {
                continue;
            };
            let first = run(target, sole_view_name, true, insufficient).unwrap_err();
            let repeated = run(target, sole_view_name, true, insufficient).unwrap_err();
            assert_eq!(first, repeated);
            assert!(matches!(
                first,
                OptimizedLiteralFoldCustodyError::SelectedLoweringBudgetExceeded {
                    required,
                    budget,
                } if budget == insufficient && !required.within(insufficient)
            ));
        }
    }
}
