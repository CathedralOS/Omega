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

#[test]
fn verified_parameter_sourced_write_only_store_reaches_assignment() {
    let checked = checked(
        r#"
            data Sink {}
            machine Sink::fill(destination: &write i32, replacement: i32) {
                destination = replacement;
            }
        "#,
    );
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Sink::fill")
        .expect("parameter-sourced store reaches verified Terminal production");
    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode parameter-sourced store semantics");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode parameter-sourced store proof bundle");
    let abstract_plan = omega_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("verified parameter-sourced store reaches target-neutral Omega");

    for native_target in [
        omega_target::NativeTarget::linux_x64(),
        omega_target::NativeTarget::linux_arm64(),
    ] {
        let target = omega_abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            native_target,
        )
        .expect("verified parameter-sourced store reaches target custody");
        let body = target
            .functions
            .iter()
            .find_map(|function| match &function.operation {
                omega_target_operations::TargetOperation::UnitBody(body) => Some(body),
                _ => None,
            })
            .expect("one target Unit body");
        let [scalar_parameter] = body.scalar_parameters.as_slice() else {
            panic!("one target scalar parameter")
        };
        let source = body
            .operations
            .iter()
            .find_map(|operation| match operation {
                omega_target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore {
                    source,
                    ..
                } => Some(source),
                _ => None,
            })
            .expect("target store retains its runtime source");
        assert!(matches!(
            source,
            omega_target_operations::TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
                parameter_index: 0,
                source_value,
                scalar_type,
            } if *source_value == scalar_parameter.value
                && psi_core::ScalarType::Integer(*scalar_type) == scalar_parameter.scalar_type
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
            .expect("corrupted target retains its runtime source");
        let omega_target_operations::TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
            parameter_index,
            ..
        } = corrupted_source
        else {
            unreachable!()
        };
        *parameter_index = 1;
        assert!(matches!(
            omega_target_operations_to_assigned_target_operations::assign_registers(&corrupted),
            Err(omega_target_operations_to_assigned_target_operations::AssignmentError::WriteOnlyPrimitiveStoreCustodyMismatch { .. })
        ));

        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target)
                .expect("parameter-sourced store reaches physical assignment");
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
            .expect("assigned store retains its runtime source");
        assert!(matches!(
            assigned_source,
            omega_assigned_target_operations::AssignedUnitWriteOnlyPrimitiveStoreSource::Parameter {
                parameter_index: 0,
                source_value,
                scalar_type,
                location: omega_assigned_target_operations::AssignedScalarLocation::Register(register),
            } if *source_value == scalar_parameter.value
                && psi_core::ScalarType::Integer(*scalar_type) == scalar_parameter.scalar_type
                && register.architecture() == native_target.architecture
        ));
        assert!(matches!(
            omega_machine_emission::emit_machine_code(&assigned),
            Err(omega_machine_emission::EmissionError::InvalidWriteOnlyPrimitiveStoreCustody(_))
        ));
    }
}

#[test]
fn verified_boolean_write_only_store_reaches_canonical_installation() {
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
            omega_target_operations::TargetUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
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
        let omega_target_operations::TargetUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
            value,
            ..
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
            omega_assigned_target_operations::AssignedUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
                defining_operation: retained_operation,
                source_value: retained_value,
                value: true,
            } if *retained_operation == defining_operation && *retained_value == source_value
        ));
        let mut corrupted_assigned = assigned.clone();
        let retained_value = corrupted_assigned
            .functions
            .iter_mut()
            .find_map(|function| match &mut function.operation {
                omega_assigned_target_operations::AssignedOperation::UnitBody(body) => {
                    body.operations.iter_mut().find_map(|operation| {
                        match operation {
                        omega_assigned_target_operations::AssignedUnitOperation::BooleanConstant {
                            value,
                            ..
                        } => Some(value),
                        _ => None,
                    }
                    })
                }
                _ => None,
            })
            .expect("assigned plan retains the Boolean definition");
        *retained_value = false;
        assert!(matches!(
            omega_machine_emission::emit_machine_code(&corrupted_assigned),
            Err(omega_machine_emission::EmissionError::InvalidWriteOnlyPrimitiveStoreCustody(_))
        ));

        let emitted = omega_machine_emission::emit_machine_code(&assigned)
            .expect("Boolean store reaches exact machine emission");
        let function = emitted
            .functions
            .iter()
            .find(|function| function.unit_write_only_primitive_stores.len() == 1)
            .expect("one machine owns the Boolean primitive store");
        let store = &function.unit_write_only_primitive_stores[0];
        assert!(matches!(
            store.source,
            omega_machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::BooleanImmediate {
                defining_operation: retained_operation,
                source_value: retained_value,
                value: true,
                definition_ordinal,
            } if retained_operation == defining_operation
                && retained_value == source_value
                && definition_ordinal < store.operation_ordinal
        ));
        assert!(!store.bytes.is_empty());
        match native_target.architecture {
            omega_target::Architecture::X86_64 => {
                assert!(
                    store
                        .bytes
                        .starts_with(&[0x49, 0xbb, 1, 0, 0, 0, 0, 0, 0, 0,])
                );
                assert!(store.bytes.ends_with(&[0x45, 0x88, 0x1a]));
            }
            omega_target::Architecture::Aarch64 => {
                assert_eq!(
                    store.bytes.first_chunk::<4>(),
                    Some(&0xd280_0030_u32.to_le_bytes())
                );
                assert_eq!(
                    store.bytes.last_chunk::<4>(),
                    Some(&0x3900_0230_u32.to_le_bytes())
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
        let omega_machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::BooleanImmediate {
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
        let mut changed_bytes = emitted.clone();
        changed_bytes
            .functions
            .iter_mut()
            .find(|candidate| candidate.machine == function.machine)
            .unwrap()
            .unit_write_only_primitive_stores[0]
            .bytes[0] ^= 1;
        assert_eq!(
            omega_image_emission::build_object_artifact(&changed_bytes),
            Err(
                omega_image_emission::ObjectError::InvalidUnitWriteOnlyPrimitiveStoreEvidence(
                    function.machine
                )
            )
        );

        let object = omega_image_emission::build_object_artifact(&emitted)
            .expect("object construction independently replays the Boolean store");
        let object_function = object
            .functions()
            .iter()
            .find(|candidate| candidate.machine == function.machine)
            .expect("object retains the Boolean store-owning function");
        assert_eq!(
            object_function.unit_write_only_primitive_stores,
            function.unit_write_only_primitive_stores
        );
        let image = omega_image_emission::emit_executable_image(&object, 3)
            .expect("replayed Boolean store reaches an executable image");
        let installation = omega_image_emission::build_installation_record(
            &image,
            psi_core::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("installation retains the replayed Boolean store");
        let installed_function = installation
            .functions()
            .iter()
            .find(|candidate| candidate.machine == function.machine)
            .expect("installation retains the Boolean store-owning function");
        assert_eq!(
            installed_function.unit_write_only_primitive_stores,
            function.unit_write_only_primitive_stores
        );
        let installation_bytes = omega_image_emission::encode_installation_record(&installation)
            .expect("encode installed Boolean store custody");
        let decoded = omega_image_emission::decode_installation_record(&installation_bytes)
            .expect("decode installed Boolean store custody");
        assert_eq!(decoded, installation);
        omega_image_emission::validate_installation_record(&decoded, &image)
            .expect("installed Boolean store rejoins the executable image");
        let installed_store_bytes = installed_function.unit_write_only_primitive_stores[0]
            .bytes
            .clone();
        let mut corrupted_installation = installation_bytes;
        let encoded_store = corrupted_installation
            .windows(installed_store_bytes.len())
            .rposition(|window| window == installed_store_bytes)
            .expect("Boolean store bytes occur in the canonical installation record");
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
