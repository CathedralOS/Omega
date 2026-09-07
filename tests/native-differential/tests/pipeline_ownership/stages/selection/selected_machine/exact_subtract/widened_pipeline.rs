//! Ordinary narrow exact arithmetic and explicit widening through physical custody.
use crate::tests::*;

#[test]
fn widened_exact_arithmetic_reaches_verified_register_and_machine_pipeline() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_widened_u8_exact_subtract_conditional(target);
        assert_ordinary_graph_custody(&staged);
        let count = staged.selected().receipt().instruction_count();
        let effects =
            analyze_machine_effects(staged.selected(), staged.register_environment()).unwrap();
        assert_eq!(effects.receipt().instruction_count(), count);
        let binaries = effects
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter(|row| matches!(row.kind, SelectedInstructionKind::ExactSubtractI64 { .. }))
            .collect::<Vec<_>>();
        assert_eq!(binaries.len(), 2);
        for row in binaries {
            assert_eq!(row.barrier, MachineBarrier::None);
            assert_eq!(row.provenance.operations.len(), 1);
            assert_eq!(row.provenance.obligations.len(), 1);
            assert_eq!(row.provenance.fuel.len(), 1);
        }
        let selected = staged.selected().plan();
        let widening = selected.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|row| {
                row.kind == SelectedInstructionKind::CopyI64
                    && !row.provenance.operations.is_empty()
            })
            .count();
        assert_eq!(widening, 2, "conversions retain separate source operations");
        let homes = stage_optimized_register_homes(
            stage_optimized_allocation_legality(
                stage_optimized_live_ranges(stage_optimized_liveness(staged).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let post = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
        assert_eq!(post.custody().instruction_count(), count);
        validate_raw_post_allocation(&homes, &post, post.machine().plan().clone()).unwrap();
    }
}
