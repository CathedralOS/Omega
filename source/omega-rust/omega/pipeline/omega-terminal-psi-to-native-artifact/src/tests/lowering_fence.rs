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
        let rejects = |candidate: &omega_machine_code::MachineCodePlan| {
            assert_eq!(
                omega_image_emission::build_object_artifact(candidate),
                Err(
                    omega_image_emission::ObjectError::InvalidUnitWriteOnlyPrimitiveStoreEvidence(
                        function.machine
                    )
                )
            );
        };
        let mut changed_type = emitted.clone();
        changed_type
            .functions
            .iter_mut()
            .find(|candidate| candidate.machine == function.machine)
            .unwrap()
            .unit_write_only_primitive_stores[0]
            .destination_type
            .identity
            .push_str("::forged");
        rejects(&changed_type);
        let mut changed_bytes = emitted.clone();
        changed_bytes
            .functions
            .iter_mut()
            .find(|candidate| candidate.machine == function.machine)
            .unwrap()
            .unit_write_only_primitive_stores[0]
            .bytes[0] ^= 1;
        rejects(&changed_bytes);
        let mut changed_home = emitted.clone();
        changed_home
            .functions
            .iter_mut()
            .find(|candidate| candidate.machine == function.machine)
            .unwrap()
            .unit_write_only_primitive_stores[0]
            .parameter_home_byte_offset += 8;
        rejects(&changed_home);
        let object = omega_image_emission::build_object_artifact(&emitted)
            .expect("object construction independently replays the store");
        let object_function = object
            .functions()
            .iter()
            .find(|candidate| candidate.machine == function.machine)
            .expect("object retains the store-owning function");
        assert_eq!(
            object_function.unit_write_only_primitive_stores,
            function.unit_write_only_primitive_stores
        );
        let image = omega_image_emission::emit_executable_image(&object, 3)
            .expect("replayed store reaches an executable image");
        let installation = omega_image_emission::build_installation_record(
            &image,
            psi_core::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("installation retains the replayed store");
        let installed_function = installation
            .functions()
            .iter()
            .find(|candidate| candidate.machine == function.machine)
            .expect("installation retains the store-owning function");
        assert_eq!(
            installed_function.unit_write_only_primitive_stores,
            function.unit_write_only_primitive_stores
        );
        let installation_bytes = omega_image_emission::encode_installation_record(&installation)
            .expect("encode the installed store custody");
        let decoded = omega_image_emission::decode_installation_record(&installation_bytes)
            .expect("decode the installed store custody");
        assert_eq!(decoded, installation);
        omega_image_emission::validate_installation_record(&decoded, &image)
            .expect("installed store rejoins the executable image");
        let installed_store_bytes = installed_function.unit_write_only_primitive_stores[0]
            .bytes
            .clone();
        let mut corrupted_installation = installation_bytes;
        let encoded_store = corrupted_installation
            .windows(installed_store_bytes.len())
            .rposition(|window| window == installed_store_bytes)
            .expect("installed store bytes occur in the canonical record");
        corrupted_installation[encoded_store] ^= 1;
        assert!(matches!(
            omega_image_emission::decode_installation_record(&corrupted_installation),
            Err(
                omega_image_emission::InstallationError::InvalidUnitWriteOnlyPrimitiveStore(
                    machine
                )
            ) if machine == function.machine
        ));
    }
}

#[test]
fn verified_boolean_write_only_store_reaches_assignment_then_stops_at_emission() {
    let checked = checked(
        r#"
            data Sink {}
            machine Sink::fill(destination: &write bool) {
                destination = true;
            }

            data Root {}
            machine Root::enter(destination: &mut bool) {
                Sink::fill(&write destination);
            }
        "#,
    );
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("Boolean write-only store reaches verified Terminal production");
    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode Boolean store semantics");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode Boolean store proof bundle");
    let abstract_plan = omega_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("verified Boolean store reaches target-neutral Omega");

    for native_target in [
        omega_target::NativeTarget::linux_x64(),
        omega_target::NativeTarget::linux_arm64(),
    ] {
        let target = omega_abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            native_target,
        )
        .expect("verified Boolean store reaches target custody");
        let body =
            target
                .functions
                .iter()
                .find_map(|function| match &function.operation {
                    omega_target_operations::TargetOperation::UnitBody(body)
                        if body.operations.iter().any(|operation| {
                            matches!(
                            operation,
                            omega_target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore {
                                ..
                            }
                        )
                        }) =>
                    {
                        Some(body)
                    }
                    _ => None,
                })
                .expect("one target body retains the Boolean store");
        let (defining_operation, source_value) = body
            .operations
            .iter()
            .find_map(|operation| match operation {
                omega_target_operations::TargetUnitOperation::BooleanConstant {
                    psi_operation,
                    result,
                    value: true,
                } => Some((*psi_operation, *result)),
                _ => None,
            })
            .expect("target custody retains the Boolean definition");
        let store = body
            .operations
            .iter()
            .find_map(|operation| match operation {
                omega_target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore {
                    destination_type,
                    destination_placement,
                    source,
                    ..
                } => Some((destination_type, destination_placement, source)),
                _ => None,
            })
            .expect("target custody retains the Boolean store");
        assert!(matches!(
            store.0.shape,
            psi_terminal::StructuralTypeShape::PrimitiveScalar(psi_core::ScalarType::Boolean)
        ));
        assert_eq!(
            store.1.shape,
            omega_calling_conventions::ValueShape::borrowed_reference(1, 1)
        );
        assert!(matches!(
            store.2,
            omega_target_operations::TargetUnitScalarArgumentSource::BooleanImmediate {
                defining_operation: retained_operation,
                source_value: retained_value,
                value: true,
            } if *retained_operation == defining_operation && *retained_value == source_value
        ));

        let mut corrupted = target.clone();
        let corrupted_source = corrupted
            .functions
            .iter_mut()
            .find_map(|function| match &mut function.operation {
                omega_target_operations::TargetOperation::UnitBody(body) => body
                    .operations
                    .iter_mut()
                    .find_map(|operation| match operation {
                        omega_target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore {
                            source,
                            ..
                        } => Some(source),
                        _ => None,
                    }),
                _ => None,
            })
            .expect("corrupted plan retains the Boolean source");
        let omega_target_operations::TargetUnitScalarArgumentSource::BooleanImmediate {
            value, ..
        } = corrupted_source
        else {
            unreachable!()
        };
        *value = false;
        assert!(matches!(
            omega_target_operations_to_assigned_target_operations::assign_registers(&corrupted),
            Err(omega_target_operations_to_assigned_target_operations::AssignmentError::WriteOnlyPrimitiveStoreCustodyMismatch { .. })
        ));

        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target)
                .expect("Boolean store reaches independently replayed physical assignment");
        let assigned_store = assigned
            .functions
            .iter()
            .find_map(|function| match &function.operation {
                omega_assigned_target_operations::AssignedOperation::UnitBody(body) => body
                    .operations
                    .iter()
                    .find_map(|operation| match operation {
                        omega_assigned_target_operations::AssignedUnitOperation::WriteOnlyPrimitiveStore {
                            destination_type,
                            destination_placement,
                            source,
                            ..
                        } => Some((destination_type, destination_placement, source)),
                        _ => None,
                    }),
                _ => None,
            })
            .expect("assigned plan retains the Boolean store");
        assert!(matches!(
            assigned_store.0.shape,
            psi_terminal::StructuralTypeShape::PrimitiveScalar(psi_core::ScalarType::Boolean)
        ));
        assert_eq!(
            assigned_store.1.shape,
            omega_calling_conventions::ValueShape::borrowed_reference(1, 1)
        );
        assert!(matches!(
            assigned_store.2,
            omega_assigned_target_operations::AssignedUnitScalarArgumentSource::BooleanImmediate {
                defining_operation: retained_operation,
                source_value: retained_value,
                value: true,
            } if *retained_operation == defining_operation && *retained_value == source_value
        ));
        assert!(matches!(
            omega_machine_emission::emit_machine_code(&assigned),
            Err(omega_machine_emission::EmissionError::UnsupportedUnitBooleanConstant(
                operation
            )) if operation == defining_operation
        ));
    }
}
