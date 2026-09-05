//! Focused write-only lowering and installation controls.

use super::*;

#[test]
fn boolean_literal_store_caller_reaches_canonical_installation() {
    let checked = checked(
        r#"
            data Sink {}
            machine Sink::fill(destination: &write bool, replacement: bool) {
                destination = replacement;
            }

            data Root {}
            machine Root::enter(destination: &mut bool) {
                Sink::fill(&write destination, false);
            }
        "#,
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("Boolean literal caller reaches verified Terminal production");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode Boolean literal caller semantics");
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode Boolean literal caller proof");
    let abstract_plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("Boolean literal caller reaches Abstract operations");

    for native_target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            native_target,
        )
        .expect("Boolean literal caller reaches Target IR");
        let target_root = target
            .functions
            .iter()
            .find(|function| function.machine == target.entry)
            .expect("Target caller is retained");
        let target_operations::TargetOperation::UnitBody(body) = &target_root.operation else {
            panic!("Target caller remains a Unit body")
        };
        let source = body
            .operations
            .iter()
            .find_map(|operation| match operation {
                target_operations::TargetUnitOperation::Call {
                    scalar_arguments, ..
                } => scalar_arguments.first().map(|argument| argument.source),
                _ => None,
            });
        assert!(matches!(
            source,
            Some(
                target_operations::TargetUnitScalarArgumentSource::BooleanImmediate {
                    value: false,
                    ..
                }
            )
        ));

        let assigned = target_operations_to_assigned_target_operations::assign_registers(&target)
            .expect("Boolean literal caller reaches physical assignment");
        let emitted = machine_emission::emit_machine_code(&assigned)
            .expect("Boolean literal caller reaches machine emission");
        let emitted_root = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("emitted Boolean literal caller is retained");
        let [call] = emitted_root.internal_unit_calls.as_slice() else {
            panic!("emitted Boolean literal caller retains one internal Unit call")
        };
        assert!(matches!(
            call.scalar_arguments.as_slice(),
            [machine_code::InternalUnitScalarCallArgumentRecord {
                source: machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
                    value: false,
                    definition_ordinal,
                    ..
                },
                byte_count,
                ..
            }] if *definition_ordinal < call.operation_ordinal && *byte_count != 0
        ));

        let mut corrupted = emitted.clone();
        let corrupted_root = corrupted
            .functions
            .iter_mut()
            .find(|function| function.machine == emitted.entry)
            .unwrap();
        let operation_ordinal = corrupted_root.internal_unit_calls[0].operation_ordinal;
        let machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
            definition_ordinal,
            ..
        } = &mut corrupted_root.internal_unit_calls[0].scalar_arguments[0].source
        else {
            unreachable!()
        };
        *definition_ordinal = operation_ordinal;
        assert_eq!(
            image_emission::build_object_artifact(&corrupted),
            Err(image_emission::ObjectError::InvalidInternalUnitCallEvidence(emitted.entry))
        );

        let object = image_emission::build_object_artifact(&emitted)
            .expect("object replay accepts Boolean literal caller custody");
        let image = image_emission::emit_executable_image(&object, 3)
            .expect("Boolean literal caller reaches an executable image");
        let installation = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("installation retains Boolean literal caller custody");
        let bytes = image_emission::encode_installation_record(&installation)
            .expect("encode installed Boolean literal caller");
        let decoded = image_emission::decode_installation_record(&bytes)
            .expect("decode installed Boolean literal caller");
        assert_eq!(decoded, installation);
        image_emission::validate_installation_record(&decoded, &image)
            .expect("installation replays Boolean literal caller custody");
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
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("Boolean write-only store reaches verified Terminal production");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode Boolean store semantics");
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode Boolean store proof bundle");
    let abstract_plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("verified Boolean store reaches target-neutral Omega");

    for native_target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            native_target,
        )
        .expect("verified Boolean store reaches target custody");
        let body = target
            .functions
            .iter()
            .find_map(|function| match &function.operation {
                target_operations::TargetOperation::UnitBody(body)
                    if body.operations.iter().any(|operation| {
                        matches!(
                            operation,
                            target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore { .. }
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
                target_operations::TargetUnitOperation::BooleanConstant {
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
                target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore {
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
            terminal_psi::StructuralTypeShape::PrimitiveScalar(
                semantic_vocabulary::ScalarType::Boolean
            )
        ));
        assert_eq!(
            store.1.shape,
            calling_conventions::ValueShape::borrowed_reference(1, 1)
        );
        assert!(matches!(
            store.2,
            target_operations::TargetUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
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
                target_operations::TargetOperation::UnitBody(body) => body
                    .operations
                    .iter_mut()
                    .find_map(|operation| match operation {
                        target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore {
                            source,
                            ..
                        } => Some(source),
                        _ => None,
                    }),
                _ => None,
            })
            .expect("corrupted plan retains the Boolean source");
        let target_operations::TargetUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
            value,
            ..
        } = corrupted_source
        else {
            unreachable!()
        };
        *value = false;
        assert!(matches!(
            target_operations_to_assigned_target_operations::assign_registers(&corrupted),
            Err(target_operations_to_assigned_target_operations::AssignmentError::WriteOnlyPrimitiveStoreCustodyMismatch { .. })
        ));

        let assigned = target_operations_to_assigned_target_operations::assign_registers(&target)
            .expect("Boolean store reaches independently replayed physical assignment");
        let assigned_store = assigned
            .functions
            .iter()
            .find_map(|function| match &function.operation {
                assigned_target_operations::AssignedOperation::UnitBody(body) => {
                    body.operations.iter().find_map(|operation| {
                        match operation {
                        assigned_target_operations::AssignedUnitOperation::WriteOnlyPrimitiveStore {
                            destination_type,
                            destination_placement,
                            source,
                            ..
                        } => Some((destination_type, destination_placement, source)),
                        _ => None,
                    }
                    })
                }
                _ => None,
            })
            .expect("assigned plan retains the Boolean store");
        assert!(matches!(
            assigned_store.0.shape,
            terminal_psi::StructuralTypeShape::PrimitiveScalar(
                semantic_vocabulary::ScalarType::Boolean
            )
        ));
        assert_eq!(
            assigned_store.1.shape,
            calling_conventions::ValueShape::borrowed_reference(1, 1)
        );
        assert!(matches!(
            assigned_store.2,
            assigned_target_operations::AssignedUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
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
                assigned_target_operations::AssignedOperation::UnitBody(body) => body
                    .operations
                    .iter_mut()
                    .find_map(|operation| match operation {
                        assigned_target_operations::AssignedUnitOperation::BooleanConstant {
                            value,
                            ..
                        } => Some(value),
                        _ => None,
                    }),
                _ => None,
            })
            .expect("assigned plan retains the Boolean definition");
        *retained_value = false;
        assert!(matches!(
            machine_emission::emit_machine_code(&corrupted_assigned),
            Err(machine_emission::EmissionError::InvalidWriteOnlyPrimitiveStoreCustody(_))
        ));

        let emitted = machine_emission::emit_machine_code(&assigned)
            .expect("Boolean store reaches exact machine emission");
        let function = emitted
            .functions
            .iter()
            .find(|function| function.unit_write_only_primitive_stores.len() == 1)
            .expect("one machine owns the Boolean primitive store");
        let store = &function.unit_write_only_primitive_stores[0];
        assert!(matches!(
            store.source,
            machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::BooleanImmediate {
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
            target::Architecture::X86_64 => {
                assert!(
                    store
                        .bytes
                        .starts_with(&[0x49, 0xbb, 1, 0, 0, 0, 0, 0, 0, 0,])
                );
                assert!(store.bytes.ends_with(&[0x45, 0x88, 0x1a]));
            }
            target::Architecture::Aarch64 => {
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
        let machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::BooleanImmediate {
            definition_ordinal,
            ..
        } = &mut changed_store.source
        else {
            unreachable!()
        };
        *definition_ordinal = store.operation_ordinal;
        assert_eq!(
            image_emission::build_object_artifact(&changed_definition_ordinal),
            Err(
                image_emission::ObjectError::InvalidUnitWriteOnlyPrimitiveStoreEvidence(
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
            image_emission::build_object_artifact(&changed_bytes),
            Err(
                image_emission::ObjectError::InvalidUnitWriteOnlyPrimitiveStoreEvidence(
                    function.machine
                )
            )
        );

        let object = image_emission::build_object_artifact(&emitted)
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
        let image = image_emission::emit_executable_image(&object, 3)
            .expect("replayed Boolean store reaches an executable image");
        let installation = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
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
        let installation_bytes = image_emission::encode_installation_record(&installation)
            .expect("encode installed Boolean store custody");
        let decoded = image_emission::decode_installation_record(&installation_bytes)
            .expect("decode installed Boolean store custody");
        assert_eq!(decoded, installation);
        image_emission::validate_installation_record(&decoded, &image)
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
            image_emission::decode_installation_record(&corrupted_installation),
            Err(
                image_emission::InstallationError::InvalidUnitWriteOnlyPrimitiveStore(
                    machine
                )
            ) if machine == function.machine
        ));
    }
}
