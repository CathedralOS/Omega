//! Typed virtual-register selection and retained custody on both architectures.

use crate::tests::*;

#[test]
fn verified_three_block_conditional_selects_typed_vregs_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_conditional(target);
        let plan = staged.selected().plan();
        assert_eq!(plan.functions.len(), 1);
        assert_eq!(plan.functions[0].blocks.len(), 3);
        assert_eq!(plan.functions[0].virtual_registers.len(), 3);
        assert_eq!(staged.selected().receipt().instruction_count(), 6);
        assert_eq!(
            staged.custody().optimization_unit(),
            staged.optimized_target().optimized().unit().identity
        );
        assert_eq!(staged.custody().fuel_schedule(), plan.fuel_schedule);
        assert_eq!(staged.legalized().receipt().target(), target);
        assert_eq!(staged.legalized().receipt().function_count(), 1);
        assert_eq!(staged.legalized().receipt().decomposition_count(), 0);
        assert_eq!(
            staged.custody().legalized(),
            staged.legalized().receipt().identity()
        );
        assert_eq!(
            staged.selected().receipt().legalized(),
            staged.legalized().receipt().identity()
        );
        assert_eq!(
            staged.custody().register_environment(),
            staged.register_environment().identity()
        );
        assert_eq!(
            staged.custody().selected(),
            staged.selected().receipt().identity()
        );
        let mut copy_tagged = plan.clone();
        copy_tagged.functions[0].blocks[1].instructions[0].kind = SelectedInstructionKind::CopyI64;
        assert_ne!(
            selected_instruction_plan_identity(&copy_tagged),
            staged.selected().receipt().identity()
        );

        let entry = &plan.functions[0].blocks[0];
        assert_eq!(
            entry.instructions[0].kind,
            SelectedInstructionKind::CompareI64Zero
        );
        assert!(entry.instructions[0].provenance.fuel.is_empty());
        let SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } = &entry.terminator
        else {
            panic!("entry must branch")
        };
        assert_eq!(
            instruction.kind,
            SelectedInstructionKind::ConditionalBranchNonZero
        );
        assert!(instruction.provenance.fuel.is_empty());
        assert_eq!(when_nonzero.fuel.len(), 1);
        assert_eq!(when_zero.fuel.len(), 1);
        assert_ne!(when_nonzero.psi_edge, when_zero.psi_edge);
        for block in &plan.functions[0].blocks[1..] {
            assert!(matches!(
                block.instructions[0].kind,
                SelectedInstructionKind::MaterializeI64 { .. }
            ));
            assert_eq!(block.instructions[0].provenance.operations.len(), 1);
            assert_eq!(block.instructions[0].provenance.fuel.len(), 1);
            let SelectedTerminator::Return { instruction, .. } = &block.terminator else {
                panic!("leaf must return")
            };
            assert!(instruction.operands[0].fixed_view.is_some());
            assert_eq!(instruction.provenance.fuel.len(), 1);
        }
    }
}
