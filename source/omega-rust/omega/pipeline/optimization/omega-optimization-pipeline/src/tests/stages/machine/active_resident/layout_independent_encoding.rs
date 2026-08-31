//! Layout-independent encoding and independent corruption rejection.

use crate::tests::{
    NativeTarget, OptimizedActiveResidentRematerializationError,
    OptimizedActiveResidentRematerializationSelectedFormEncodingError,
    OptimizedSelectedFormEncodingError, SelectedFormEncodingState,
    stage_optimized_active_resident_rematerialization_selected_form_encoding,
    staged_active_resident_rematerialization_and_machine,
    validate_optimized_active_resident_rematerialization_selected_form_encoding,
};

#[test]
fn active_resident_rematerialization_reaches_layout_independent_encoding_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (source, machine) = staged_active_resident_rematerialization_and_machine(target);
        let transformed_selected = source.rematerialization().receipt().transformed_selected();
        let machine_root = machine.machine().receipt().identity();
        let machine_row_count = machine.custody().instruction_count();
        let rematerialization = source.custody();
        let fresh_materialize = source.rematerialization().plan().functions[0]
            .action
            .as_ref()
            .unwrap()
            .fresh_materialize;

        let staged = stage_optimized_active_resident_rematerialization_selected_form_encoding(
            source, machine,
        )
        .unwrap();
        assert_eq!(staged.encoding().selected(), transformed_selected);
        assert_eq!(staged.encoding().machine(), machine_root);
        assert_eq!(staged.custody().rematerialization(), rematerialization);
        assert_eq!(staged.custody().machine(), staged.machine().custody());
        assert_eq!(
            staged.custody().transformed_selected(),
            transformed_selected
        );
        assert_eq!(staged.custody().encoding(), staged.encoding().identity());
        assert_eq!(staged.custody().row_count(), machine_row_count);
        assert_eq!(
            staged.custody().encoded_count() + staged.custody().deferred_count(),
            machine_row_count
        );
        assert_eq!(staged.custody().deferred_count(), 1);
        assert!(staged.encoding().rows().iter().all(|row| match &row.state {
            SelectedFormEncodingState::Encoded { bytes, .. } => !bytes.is_empty(),
            SelectedFormEncodingState::DeferredControl { .. } => true,
        }));
        let fresh_row = staged
            .encoding()
            .rows()
            .iter()
            .find(|row| row.instruction == fresh_materialize)
            .expect("fresh rematerialization must reach the encoder roster");
        assert_eq!(
            fresh_row.alternative.family,
            omega_selected_instructions::MachineAlternativeFamily::MaterializeI64
        );
        assert!(matches!(
            &fresh_row.state,
            SelectedFormEncodingState::Encoded { bytes, .. } if !bytes.is_empty()
        ));
        assert_eq!(
            validate_optimized_active_resident_rematerialization_selected_form_encoding(&staged,)
                .unwrap(),
            staged.custody().clone()
        );
    }
}

#[test]
fn active_resident_rematerialization_encoding_rejects_detached_or_corrupt_custody() {
    let (mut corrupt_source, machine) =
        staged_active_resident_rematerialization_and_machine(NativeTarget::linux_x64());
    crate::stages::machine::active_resident_rematerialization::corrupt_active_resident_rematerialization_custody_for_test(
        &mut corrupt_source,
    );
    assert!(matches!(
        stage_optimized_active_resident_rematerialization_selected_form_encoding(
            corrupt_source,
            machine,
        ),
        Err(
            OptimizedActiveResidentRematerializationSelectedFormEncodingError::Rematerialization(
                OptimizedActiveResidentRematerializationError::ReceiptMismatch
            )
        )
    ));

    let (x86_source, _) =
        staged_active_resident_rematerialization_and_machine(NativeTarget::linux_x64());
    let (_, arm_machine) =
        staged_active_resident_rematerialization_and_machine(NativeTarget::linux_arm64());
    assert!(matches!(
        stage_optimized_active_resident_rematerialization_selected_form_encoding(
            x86_source,
            arm_machine,
        ),
        Err(OptimizedActiveResidentRematerializationSelectedFormEncodingError::Machine(_))
    ));

    let (source, machine) =
        staged_active_resident_rematerialization_and_machine(NativeTarget::linux_x64());
    let mut staged =
        stage_optimized_active_resident_rematerialization_selected_form_encoding(source, machine)
            .unwrap();
    crate::stages::encoding::active_resident_selected_form_encoding::corrupt_active_resident_selected_form_encoding_byte_for_test(
        &mut staged,
    );
    assert_eq!(
        validate_optimized_active_resident_rematerialization_selected_form_encoding(&staged),
        Err(
            OptimizedActiveResidentRematerializationSelectedFormEncodingError::Encoding(
                OptimizedSelectedFormEncodingError::ArtifactMismatch
            )
        )
    );
}
