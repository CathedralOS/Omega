//! Rematerialized programs use the ordinary encoder and replay boundaries.
use crate::tests::*;

#[test]
fn active_resident_rematerialization_reaches_layout_independent_encoding_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (source, machine) = staged_active_resident_rematerialization_and_machine(target);
        let fresh = source.rematerialization().plan().functions[0]
            .action
            .as_ref()
            .unwrap()
            .fresh_materialize;
        let current = source.replay_allocation().unwrap();
        validate_optimized_post_allocation_machine_plan_custody(&current, &machine).unwrap();
        let physical = current.register_environment().physical();
        let encoding = stage_optimized_layout_independent_selected_form_encoding(
            current.selected(),
            &machine,
            physical,
        )
        .unwrap();
        assert_eq!(
            encoding.selected(),
            source.rematerialization().receipt().transformed_selected()
        );
        assert_eq!(encoding.machine(), machine.machine().receipt().identity());
        assert_eq!(
            current.evidence(),
            &AllocationEvidence::ActiveResidentRematerialization(source.custody())
        );
        assert_eq!(encoding.rows().len(), machine.custody().instruction_count());
        assert_eq!(
            encoding
                .rows()
                .iter()
                .filter(|row| matches!(
                    row.state,
                    SelectedFormEncodingState::DeferredControl { .. }
                ))
                .count(),
            1
        );
        assert!(encoding.rows().iter().all(|row| match &row.state {
            SelectedFormEncodingState::Encoded { bytes, .. } => !bytes.is_empty(),
            SelectedFormEncodingState::UnresolvedInternalMachineCall { .. } => false,
            SelectedFormEncodingState::DeferredControl { .. } => true,
        }));
        let fresh_row = encoding
            .rows()
            .iter()
            .find(|row| row.instruction == fresh)
            .unwrap();
        assert_eq!(
            fresh_row.alternative.family,
            selected_instructions::MachineAlternativeFamily::MaterializeI64
        );
        assert!(
            matches!(&fresh_row.state, SelectedFormEncodingState::Encoded { bytes, .. } if !bytes.is_empty())
        );
        validate_optimized_layout_independent_selected_form_encoding(
            current.selected(),
            &machine,
            physical,
            &encoding,
        )
        .unwrap();
        let retained = encoding.shared_program();
        assert!(std::ptr::eq(retained.as_ref(), encoding.program()));
        let expected_identity = encoding.identity();
        let expected_rows = encoding.rows().len();
        drop(encoding);
        drop(current);
        drop(machine);
        drop(source);
        assert_eq!(retained.identity, expected_identity);
        assert_eq!(retained.rows.len(), expected_rows);
        assert!(retained.rows.iter().any(|row| matches!(
            &row.state,
            SelectedFormEncodingState::Encoded { bytes, .. } if !bytes.is_empty()
        )));
    }
}

#[test]
fn active_resident_rematerialization_encoding_rejects_detached_or_corrupt_custody() {
    let (mut corrupt_source, _) =
        staged_active_resident_rematerialization_and_machine(NativeTarget::linux_x64());
    crate::corrupt_active_resident_rematerialization_custody_for_test(&mut corrupt_source);
    assert!(matches!(
        corrupt_source.replay_allocation(),
        Err(AllocationReplayError::ActiveResidentRematerialization(
            OptimizedActiveResidentRematerializationError::ReceiptMismatch
        ))
    ));

    let (source, machine) =
        staged_active_resident_rematerialization_and_machine(NativeTarget::linux_x64());
    let (_, foreign_machine) =
        staged_active_resident_rematerialization_and_machine(NativeTarget::linux_arm64());
    let current = source.replay_allocation().unwrap();
    assert!(
        validate_optimized_post_allocation_machine_plan_custody(&current, &foreign_machine)
            .is_err()
    );
    let physical = current.register_environment().physical();
    let mut encoding = stage_optimized_layout_independent_selected_form_encoding(
        current.selected(),
        &machine,
        physical,
    )
    .unwrap();
    let row = encoding
        .rows_mut()
        .iter_mut()
        .find(|row| matches!(row.state, SelectedFormEncodingState::Encoded { .. }))
        .unwrap();
    let SelectedFormEncodingState::Encoded { bytes, .. } = &mut row.state else {
        unreachable!()
    };
    bytes[0] ^= 1;
    assert_eq!(
        validate_optimized_layout_independent_selected_form_encoding(
            current.selected(),
            &machine,
            physical,
            &encoding
        ),
        Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
    );
}
