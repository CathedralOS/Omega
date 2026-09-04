//! Focused write-only lowering and installation controls.

use super::*;

#[test]
fn verified_write_only_ieee_float_store_reaches_canonical_installation() {
    let checked = checked(
        r#"
            data Sink {}
            machine Sink::fill(destination: &write f64) {
                destination = 1.25f64;
            }

            data Root {}
            machine Root::enter(destination: &mut f64) {
                Sink::fill(&write destination);
            }
        "#,
    );
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("write-only IEEE float store reaches verified Terminal production");
    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode IEEE float store semantics");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode IEEE float store proof bundle");
    let abstract_plan = omega_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("verified IEEE float store reaches target-neutral Omega");

    for native_target in [
        omega_target::NativeTarget::linux_x64(),
        omega_target::NativeTarget::linux_arm64(),
    ] {
        let target = omega_abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            native_target,
        )
        .expect("verified IEEE float store reaches target custody");
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
                .expect("one target body retains the IEEE float store");
        let (defining_operation, source_value) = body
            .operations
            .iter()
            .find_map(|operation| match operation {
                omega_target_operations::TargetUnitOperation::IeeeFloatConstant {
                    psi_operation,
                    result,
                    value: psi_core::IeeeFloatValue::Binary64(0x3ff4_0000_0000_0000),
                } => Some((*psi_operation, *result)),
                _ => None,
            })
            .expect("target custody retains the IEEE float definition");
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
            .expect("target custody retains the IEEE float store");
        assert!(matches!(
            store.0.shape,
            psi_terminal::StructuralTypeShape::PrimitiveScalar(psi_core::ScalarType::IeeeFloat(
                psi_core::IeeeFloatFormat::Binary64
            ))
        ));
        assert_eq!(
            store.1.shape,
            omega_calling_conventions::ValueShape::borrowed_reference(8, 8)
        );
        assert!(matches!(
            store.2,
            omega_target_operations::TargetUnitWriteOnlyPrimitiveStoreSource::IeeeFloatImmediate {
                defining_operation: retained_operation,
                source_value: retained_value,
                value: psi_core::IeeeFloatValue::Binary64(0x3ff4_0000_0000_0000),
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
            .expect("corrupted plan retains the IEEE float source");
        let omega_target_operations::TargetUnitWriteOnlyPrimitiveStoreSource::IeeeFloatImmediate {
            value,
            ..
        } = corrupted_source
        else {
            unreachable!()
        };
        *value = psi_core::IeeeFloatValue::Binary64(0);
        assert!(matches!(
            omega_target_operations_to_assigned_target_operations::assign_registers(&corrupted),
            Err(omega_target_operations_to_assigned_target_operations::AssignmentError::WriteOnlyPrimitiveStoreCustodyMismatch { .. })
        ));

        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target)
                .expect("IEEE float store reaches independently replayed physical assignment");
        let assigned_source = assigned
            .functions
            .iter()
            .find_map(|function| match &function.operation {
                omega_assigned_target_operations::AssignedOperation::UnitBody(body) => body
                    .operations
                    .iter()
                    .find_map(|operation| match operation {
                        omega_assigned_target_operations::AssignedUnitOperation::WriteOnlyPrimitiveStore {
                            source,
                            ..
                        } => Some(source),
                        _ => None,
                    }),
                _ => None,
            })
            .expect("assigned plan retains the IEEE float store source");
        assert!(matches!(
            assigned_source,
            omega_assigned_target_operations::AssignedUnitWriteOnlyPrimitiveStoreSource::IeeeFloatImmediate {
                defining_operation: retained_operation,
                source_value: retained_value,
                value: psi_core::IeeeFloatValue::Binary64(0x3ff4_0000_0000_0000),
            } if *retained_operation == defining_operation && *retained_value == source_value
        ));
        let emitted = omega_machine_emission::emit_machine_code(&assigned)
            .expect("IEEE float store reaches exact machine emission");
        let function = emitted
            .functions
            .iter()
            .find(|function| function.unit_write_only_primitive_stores.len() == 1)
            .expect("one machine owns the IEEE float primitive store");
        let store = &function.unit_write_only_primitive_stores[0];
        assert!(matches!(
            store.source,
            omega_machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IeeeFloatImmediate {
                defining_operation: retained_operation,
                source_value: retained_value,
                value: psi_core::IeeeFloatValue::Binary64(0x3ff4_0000_0000_0000),
                definition_ordinal,
            } if retained_operation == defining_operation
                && retained_value == source_value
                && definition_ordinal < store.operation_ordinal
        ));
        match native_target.architecture {
            omega_target::Architecture::X86_64 => {
                assert!(
                    store
                        .bytes
                        .starts_with(&[0x49, 0xbb, 0, 0, 0, 0, 0, 0, 0xf4, 0x3f,])
                );
                assert!(store.bytes.ends_with(&[0x4d, 0x89, 0x1a]));
            }
            omega_target::Architecture::Aarch64 => {
                assert_eq!(
                    store.bytes.first_chunk::<4>(),
                    Some(&0xd280_0010_u32.to_le_bytes())
                );
                assert_eq!(
                    store.bytes.last_chunk::<4>(),
                    Some(&0xf900_0230_u32.to_le_bytes())
                );
            }
        }
        assert_eq!(
            function
                .bytes
                .get(store.code_offset..store.code_offset + store.byte_count),
            Some(store.bytes.as_slice())
        );

        let mut changed_definition_ordinal = emitted.clone();
        let changed_store = &mut changed_definition_ordinal
            .functions
            .iter_mut()
            .find(|candidate| candidate.machine == function.machine)
            .unwrap()
            .unit_write_only_primitive_stores[0];
        let omega_machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::IeeeFloatImmediate {
            definition_ordinal,
            ..
        } = &mut changed_store.source
        else {
            unreachable!()
        };
        *definition_ordinal = store.operation_ordinal;
        assert_eq!(
            omega_image_emission::build_object_artifact(&changed_definition_ordinal),
            Err(
                omega_image_emission::ObjectError::InvalidUnitWriteOnlyPrimitiveStoreEvidence(
                    function.machine
                )
            )
        );

        let object = omega_image_emission::build_object_artifact(&emitted)
            .expect("object construction independently replays the IEEE float store");
        let image = omega_image_emission::emit_executable_image(&object, 3)
            .expect("replayed IEEE float store reaches an executable image");
        let installation = omega_image_emission::build_installation_record(
            &image,
            psi_core::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("installation retains the replayed IEEE float store");
        let installed_function = installation
            .functions()
            .iter()
            .find(|candidate| candidate.machine == function.machine)
            .expect("installation retains the IEEE float store-owning function");
        assert_eq!(
            installed_function.unit_write_only_primitive_stores,
            function.unit_write_only_primitive_stores
        );
        let installation_bytes = omega_image_emission::encode_installation_record(&installation)
            .expect("encode installed IEEE float store custody");
        let decoded = omega_image_emission::decode_installation_record(&installation_bytes)
            .expect("decode installed IEEE float store custody");
        assert_eq!(decoded, installation);
        omega_image_emission::validate_installation_record(&decoded, &image)
            .expect("installed IEEE float store rejoins the executable image");

        let raw_bits = 0x3ff4_0000_0000_0000_u64.to_le_bytes();
        let mut corrupted_installation = installation_bytes;
        let encoded_source = corrupted_installation
            .windows(raw_bits.len())
            .rposition(|window| window == raw_bits)
            .expect("IEEE float raw bits occur in the canonical installation record");
        corrupted_installation[encoded_source] ^= 1;
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
