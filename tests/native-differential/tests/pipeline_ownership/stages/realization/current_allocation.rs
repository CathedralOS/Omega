use crate::tests::*;
use selected_instructions_to_register_homes::RetainedAllocation;

pub(super) fn allocation(target: NativeTarget, relaxation: bool) -> RetainedAllocation {
    let mut selections = vec![Optimization::CopyPropagation];
    if relaxation {
        selections.push(Optimization::X86RelaxConditionalBranchesToRel8V1);
    }
    let selected = staged_exact_add_conditional_with_selections(
        target,
        OptimizationSelections::new(selections).unwrap(),
        selected_lowering_budget(),
    );
    let ranges = stage_optimized_live_ranges(stage_optimized_liveness(selected).unwrap()).unwrap();
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    stage_optimized_register_homes(legality)
        .unwrap()
        .try_into()
        .unwrap()
}
