use crate::tests::*;

use super::fixture::staged_not_equal_zero_parameter;

#[test]
fn not_equal_zero_selected_compare_branch_and_successor_corruption_fail_closed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_not_equal_zero_parameter(target);
        let comparison = staged.selected().plan().functions[0].blocks[0]
            .instructions
            .iter()
            .position(|row| row.kind == SelectedInstructionKind::CompareI64Zero)
            .unwrap();

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[0].instructions[comparison].kind =
            SelectedInstructionKind::CompareI64;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
        ));

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::ConditionalBranch { instruction, .. } =
            &mut corrupted.functions[0].blocks[0].terminator
        else {
            panic!("fixture must retain conditional control")
        };
        instruction.kind = SelectedInstructionKind::ConditionalBranchU64LessThan;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
        ));

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } = &mut corrupted.functions[0].blocks[0].terminator
        else {
            panic!("fixture must retain conditional control")
        };
        std::mem::swap(when_nonzero, when_zero);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::SourceCustodyMismatch)
        ));
    }
}

#[test]
fn not_equal_zero_selected_provenance_constraint_and_register_corruption_fail_closed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_not_equal_zero_parameter(target);
        let comparison = staged.selected().plan().functions[0].blocks[0]
            .instructions
            .iter()
            .position(|row| row.kind == SelectedInstructionKind::CompareI64Zero)
            .unwrap();

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[0].instructions[comparison]
            .provenance
            .operations
            .swap(0, 1);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
        ));

        let mut corrupted = staged.selected().plan().clone();
        let SelectedTerminator::ConditionalBranch { instruction, .. } =
            &mut corrupted.functions[0].blocks[0].terminator
        else {
            panic!("fixture must retain conditional control")
        };
        instruction.provenance.fuel[0].units += 1;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
                | Err(SelectedInstructionError::ProvenancePartitionMismatch { function: 0 })
        ));

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[0].instructions[comparison].constraint =
            staged.register_environment().selected_keys().copy_i64;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
                | Err(SelectedInstructionError::ConstraintOperandMismatch {
                    function: 0,
                    instruction: 0
                })
        ));

        let mut corrupted = staged.selected().plan().clone();
        let VirtualRegisterOrigin::EntryParameter {
            parameter_index, ..
        } = &mut corrupted.functions[0].virtual_registers[0].origin
        else {
            panic!("fixture must retain parameter virtual-register custody")
        };
        *parameter_index = 1;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
        ));
    }
}
