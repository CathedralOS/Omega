//! Verified write-only storage reaches the explicit physical-lowering fence.

use crate::tests::fixtures::checked_source::checked;

#[test]
fn verified_write_only_primitive_store_stops_at_physical_lowering_fence() {
    let checked = checked(
        r#"
            data Sink {}
            machine Sink::fill(destination: &write i32) {
                destination = 2;
            }

            data Root {}
            machine Root::enter(destination: &mut i32) {
                Sink::fill(&write destination);
            }
        "#,
    );
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("write-only store reaches verified Terminal production");
    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode write-only store semantics");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode write-only store proof bundle");
    let abstract_plan = omega_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("verified write-only store reaches target-neutral Omega");
    let error = omega_abstract_operations_to_target_operations::lower_to_target_operations(
        &abstract_plan,
        omega_target::NativeTarget::linux_x64(),
    )
    .expect_err("native lowering has no physical store custody yet");
    assert!(matches!(
        error,
        omega_abstract_operations_to_target_operations::LoweringError::UnsupportedWriteOnlyPrimitiveStore { .. }
    ), "unexpected lowering fence: {error:?}");
}
