use crate::tests::*;

use super::fixture::*;

const EXACT_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 19,
    candidates: 8,
    validation_steps: 123,
    commits: 6,
    iterations: 9,
};

fn exact_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(
        EXACT_USAGE.rule_evaluations,
        EXACT_USAGE.candidates,
        EXACT_USAGE.validation_steps,
        EXACT_USAGE.commits,
        EXACT_USAGE.iterations,
    )
    .unwrap()
}

fn first_over_boundary(axis: usize) -> OptimizationWorkBudget {
    let exact = exact_budget();
    let mut fields = [
        exact.rule_evaluations(),
        exact.candidates(),
        exact.validation_steps(),
        exact.commits(),
        exact.iterations(),
    ];
    fields[axis] -= 1;
    OptimizationWorkBudget::new(fields[0], fields[1], fields[2], fields[3], fields[4]).unwrap()
}

#[test]
fn exact_subtract_rule_pins_exact_aggregate_work_and_all_five_first_over_boundaries() {
    for (target, sole_view_name) in targets() {
        let reference = run(target, sole_view_name, true, selected_lowering_budget()).unwrap();
        assert_eq!(reference.custody().usage(), EXACT_USAGE);

        let exact = exact_budget();
        let exact_run = run(target, sole_view_name, true, exact).unwrap();
        assert_eq!(exact_run.custody().usage(), EXACT_USAGE);
        assert_eq!(exact_run.custody().budget(), exact);

        for axis in 0..5 {
            let insufficient = first_over_boundary(axis);
            let first = run(target, sole_view_name, true, insufficient).unwrap_err();
            let repeated = run(target, sole_view_name, true, insufficient).unwrap_err();
            assert_eq!(first, repeated);
            assert!(matches!(
                first,
                OptimizedLiteralFoldCustodyError::SelectedLoweringBudgetExceeded {
                    required,
                    budget,
                } if budget == insufficient
                    && !required.within(insufficient)
                    && required.within(exact)
            ));
        }
    }
}
