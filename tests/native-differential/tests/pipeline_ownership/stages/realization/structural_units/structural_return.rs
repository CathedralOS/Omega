//! An unused owned parameter keeps its semantic signature and ABI custody,
//! without a physical observer, descriptor, copy, or register. Selection must
//! preserve that distinction rather than reject every structural signature.

use crate::tests::*;

#[test]
fn structural_unit_return_selects_without_materializing_unused_owned_input() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (semantic, proof) = structurally_parameterized_unit_return_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target = lower_optimized_to_target_operations(optimized, target).unwrap();

        let selected = stage_optimized_instruction_selection(target)
            .expect("unused owned input must reach ordinary instruction selection");
        validate_optimized_selection_custody(
            selected.optimized_target(),
            selected.register_environment(),
            selected.legalized(),
            selected.selected(),
        )
        .expect("selection must retain independently checked source and target custody");

        let [function] = selected.selected().plan().functions.as_slice() else {
            panic!("one ordinary selected function");
        };
        assert!(
            selected
                .selected()
                .plan()
                .projected_structural_call_returns
                .is_empty()
        );
        let contract = function
            .structural
            .as_ref()
            .expect("retained owned signature");
        let [parameter] = contract.parameters.as_slice() else {
            panic!("one retained owned parameter");
        };
        assert_eq!(
            parameter.semantic,
            selected.optimized_target().optimized().plan().functions[0].structural_parameters[0]
        );
        assert_eq!(parameter.semantic.access, StructuralAccess::Owned);
        assert_eq!(
            parameter.semantic.multiplicity,
            StructuralMultiplicity::Unrestricted
        );
        assert_eq!(
            function.structural,
            selected.legalized().plan().scalar_functions[0].structural
        );
        assert!(function.virtual_registers.is_empty());
        assert!(function.local_storage_slots.is_empty());
        assert!(function.outgoing_arguments.is_empty());
        assert!(function.memory_accesses.is_empty());
        assert!(function.calls.is_empty());
        let [block] = function.blocks.as_slice() else {
            panic!("one source return block");
        };
        assert!(block.instructions.is_empty());
        let SelectedTerminator::Return {
            instruction,
            psi_return_edge,
        } = &block.terminator
        else {
            panic!("ordinary Unit return");
        };
        assert_eq!(*psi_return_edge, EdgeId::new(3_503).unwrap());
        assert!(matches!(
            instruction.kind,
            SelectedInstructionKind::ReturnUnit
        ));
        assert!(instruction.operands.is_empty());

        // No runtime use does not authorize deleting the semantic/ABI parameter.
        let mut missing_parameter = selected.selected().plan().clone();
        missing_parameter.functions[0]
            .structural
            .as_mut()
            .unwrap()
            .parameters
            .clear();
        assert!(validate_raw_selection(&selected, missing_parameter).is_err());
    }
}
