//! The ordinary layout stage preserves rematerialized instructions and rejects corruption.
use crate::tests::*;

#[test]
fn active_resident_rematerialization_reaches_resolved_layout_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (source, machine) = staged_active_resident_rematerialization_and_machine(target);
        let fresh = source.rematerialization().plan().functions[0]
            .action
            .as_ref()
            .unwrap()
            .fresh_materialize;
        let current = source.replay_allocation().unwrap();
        let physical = current.register_environment().physical();
        let encoding = stage_optimized_layout_independent_selected_form_encoding(
            current.selected(),
            &machine,
            physical,
        )
        .unwrap();
        let layout = stage_optimized_resolved_selected_form_layout(
            current.selected(),
            &machine,
            physical,
            &encoding,
        )
        .unwrap();
        assert_eq!(layout.selected(), encoding.selected());
        assert_eq!(layout.machine(), encoding.machine());
        assert_eq!(layout.pre_layout(), encoding.identity());
        assert_eq!(layout.target(), target);
        assert_eq!(
            layout.policy(),
            SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1
        );
        assert_eq!(layout.functions().len(), 1);
        assert_eq!(layout.functions()[0].blocks.len(), 3);
        let rows = layout
            .functions()
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), encoding.rows().len());
        assert_eq!(rows.iter().filter(|row| row.branch.is_some()).count(), 1);
        let fresh_row = rows.iter().find(|row| row.instruction == fresh).unwrap();
        assert_eq!(
            fresh_row.alternative.family,
            omega_selected_instructions::MachineAlternativeFamily::MaterializeI64
        );
        assert!(!fresh_row.bytes.is_empty());
        assert_eq!(
            rows.iter().map(|row| row.bytes.len() as u64).sum::<u64>(),
            layout
                .functions()
                .iter()
                .map(|function| function.byte_count)
                .sum::<u64>()
        );
        validate_optimized_resolved_selected_form_layout(
            current.selected(),
            &machine,
            physical,
            &encoding,
            &layout,
        )
        .unwrap();
    }
}

#[test]
fn active_resident_resolved_layout_rejects_pre_layout_layout_and_receipt_mutation() {
    let mut corrupt_encoding =
        staged_active_resident_allocation_recovery_realization(NativeTarget::linux_x64());
    corrupt_allocation_recovery_realization_encoding_for_test(&mut corrupt_encoding);
    assert!(matches!(
        validate_allocation_recovery_function_relative_realization(&corrupt_encoding),
        Err(
            AllocationRecoveryFunctionRelativeRealizationError::Encoding(
                OptimizedSelectedFormEncodingError::ArtifactMismatch
            )
        )
    ));

    let mut corrupt_layout =
        staged_active_resident_allocation_recovery_realization(NativeTarget::linux_x64());
    corrupt_allocation_recovery_realization_layout_for_test(&mut corrupt_layout);
    assert!(matches!(
        validate_allocation_recovery_function_relative_realization(&corrupt_layout),
        Err(AllocationRecoveryFunctionRelativeRealizationError::Layout(
            OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch
        ))
    ));

    let mut corrupt_receipt =
        staged_active_resident_allocation_recovery_realization(NativeTarget::linux_x64());
    corrupt_allocation_recovery_realization_custody_for_test(&mut corrupt_receipt);
    assert!(matches!(
        validate_allocation_recovery_function_relative_realization(&corrupt_receipt),
        Err(AllocationRecoveryFunctionRelativeRealizationError::ReceiptMismatch)
    ));
}
