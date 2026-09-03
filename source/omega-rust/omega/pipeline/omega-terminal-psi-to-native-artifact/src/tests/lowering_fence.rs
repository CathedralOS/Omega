//! Verified write-only storage reaches exact cross-target machine emission.

use crate::tests::fixtures::checked_source::checked;

#[test]
fn verified_write_only_primitive_store_reaches_exact_machine_emission() {
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
    let x64_target = omega_abstract_operations_to_target_operations::lower_to_target_operations(
        &abstract_plan,
        omega_target::NativeTarget::linux_x64(),
    )
    .expect("verified whole-root store reaches target custody");
    let mut corrupted = x64_target.clone();
    let operation = corrupted
        .functions
        .iter_mut()
        .find_map(|function| match &mut function.operation {
            omega_target_operations::TargetOperation::UnitBody(body) => {
                body.operations.iter_mut().find(|operation| {
                    matches!(
                        operation,
                        omega_target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore { .. }
                    )
                })
            }
            _ => None,
        })
        .expect("target plan retains the write-only store");
    let omega_target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore {
        destination_type,
        ..
    } = operation
    else {
        unreachable!()
    };
    destination_type.identity.push_str("::forged");
    assert!(matches!(
        omega_target_operations_to_assigned_target_operations::assign_registers(&corrupted),
        Err(omega_target_operations_to_assigned_target_operations::AssignmentError::WriteOnlyPrimitiveStoreCustodyMismatch { .. })
    ));

    for target in [
        omega_target::NativeTarget::linux_x64(),
        omega_target::NativeTarget::linux_arm64(),
    ] {
        let target = omega_abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            target,
        )
        .expect("verified whole-root store reaches target custody");
        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target)
                .expect("exact target store reaches independently replayed physical assignment");
        let emitted = omega_machine_emission::emit_machine_code(&assigned)
            .expect("exact whole-root store reaches physical machine emission");
        let function = emitted
            .functions
            .iter()
            .find(|function| function.unit_write_only_primitive_stores.len() == 1)
            .expect("one machine owns the write-only primitive store");
        let [store] = function.unit_write_only_primitive_stores.as_slice() else {
            unreachable!()
        };
        assert_eq!(store.destination.structural_type, store.destination_type.id);
        assert!(matches!(
            store.destination.access,
            psi_terminal::StructuralAccess::WriteOnlyBorrow
        ));
        assert!(matches!(
            store.destination_type.shape,
            psi_terminal::StructuralTypeShape::PrimitiveScalar(psi_core::ScalarType::Integer(_))
        ));
        let home = function
            .unit_parameter_homes
            .iter()
            .find(|home| home.place == store.destination.place)
            .expect("store destination retains its exact parameter home");
        assert_eq!(store.destination_placement, home.source);
        assert_eq!(store.parameter_home_byte_offset, home.byte_offset);
        assert_eq!(store.parameter_home_indirect, home.indirect);
        assert!(!store.bytes.is_empty());
        assert_eq!(
            &function.bytes[store.code_offset..store.code_offset + store.byte_count],
            store.bytes
        );
        assert!(matches!(
            omega_image_emission::build_object_artifact(&emitted),
            Err(
                omega_image_emission::ObjectError::UnsupportedUnitWriteOnlyPrimitiveStore(
                    machine
                )
            ) if machine == function.machine
        ));
    }
}
