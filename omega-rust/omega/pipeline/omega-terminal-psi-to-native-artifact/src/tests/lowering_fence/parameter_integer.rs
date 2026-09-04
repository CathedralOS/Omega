//! Focused write-only lowering and installation controls.

use super::*;

#[test]
fn verified_parameter_sourced_write_only_store_reaches_canonical_installation() {
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
                && *scalar_type == scalar_parameter.scalar_type
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
                && *scalar_type == scalar_parameter.scalar_type
                && register.architecture() == native_target.architecture
        ));
        let mut corrupted_assigned = assigned.clone();
        let corrupted_location = corrupted_assigned
            .functions
            .iter_mut()
            .find_map(|function| match &mut function.operation {
                omega_assigned_target_operations::AssignedOperation::UnitBody(body) => body
                    .operations
                    .iter_mut()
                    .find_map(|operation| match operation {
                        omega_assigned_target_operations::AssignedUnitOperation::WriteOnlyPrimitiveStore {
                            source: omega_assigned_target_operations::AssignedUnitWriteOnlyPrimitiveStoreSource::Parameter {
                                location,
                                ..
                            },
                            ..
                        } => Some(location),
                        _ => None,
                    }),
                _ => None,
            })
            .expect("assigned store retains its incoming location");
        *corrupted_location = match native_target.architecture {
            omega_target::Architecture::X86_64 => {
                omega_assigned_target_operations::AssignedScalarLocation::Register(
                    omega_target_operations::MachineRegister::X86Rax,
                )
            }
            omega_target::Architecture::Aarch64 => {
                omega_assigned_target_operations::AssignedScalarLocation::Register(
                    omega_target_operations::MachineRegister::Aarch64X(1),
                )
            }
        };
        assert!(matches!(
            omega_machine_emission::emit_machine_code(&corrupted_assigned),
            Err(omega_machine_emission::EmissionError::InvalidWriteOnlyPrimitiveStoreCustody(_))
        ));
        let emitted = omega_machine_emission::emit_machine_code(&assigned)
            .expect("parameter-sourced store reaches exact machine emission");
        let function = emitted
            .functions
            .iter()
            .find(|function| function.unit_write_only_primitive_stores.len() == 1)
            .expect("one machine owns the parameter-sourced primitive store");
        let store = &function.unit_write_only_primitive_stores[0];
        let expected_register = match native_target.architecture {
            omega_target::Architecture::X86_64 => omega_target_operations::MachineRegister::X86Rdi,
            omega_target::Architecture::Aarch64 => {
                omega_target_operations::MachineRegister::Aarch64X(0)
            }
        };
        assert!(matches!(
            store.source,
            omega_machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                parameter_index: 0,
                source_value,
                scalar_type,
                location: omega_machine_code::UnitScalarParameterLocationRecord::Register(register),
            } if source_value == scalar_parameter.value
                && scalar_type == scalar_parameter.scalar_type
                && register == expected_register
        ));
        match native_target.architecture {
            omega_target::Architecture::X86_64 => {
                assert!(store.bytes.ends_with(&[0x41, 0x89, 0x3a]));
            }
            omega_target::Architecture::Aarch64 => {
                assert_eq!(
                    store.bytes.last_chunk::<4>(),
                    Some(&0xb900_0220_u32.to_le_bytes())
                );
            }
        }
        assert_eq!(
            function
                .bytes
                .get(store.code_offset..store.code_offset + store.byte_count),
            Some(store.bytes.as_slice())
        );

        let mut changed_location = emitted.clone();
        let changed_source = &mut changed_location
            .functions
            .iter_mut()
            .find(|candidate| candidate.machine == function.machine)
            .unwrap()
            .unit_write_only_primitive_stores[0]
            .source;
        let omega_machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
            location, ..
        } = changed_source
        else {
            unreachable!()
        };
        *location = match native_target.architecture {
            omega_target::Architecture::X86_64 => {
                omega_machine_code::UnitScalarParameterLocationRecord::Register(
                    omega_target_operations::MachineRegister::X86Rax,
                )
            }
            omega_target::Architecture::Aarch64 => {
                omega_machine_code::UnitScalarParameterLocationRecord::Register(
                    omega_target_operations::MachineRegister::Aarch64X(1),
                )
            }
        };
        assert_eq!(
            omega_image_emission::build_object_artifact(&changed_location),
            Err(
                omega_image_emission::ObjectError::InvalidUnitWriteOnlyPrimitiveStoreEvidence(
                    function.machine
                )
            )
        );

        let object = omega_image_emission::build_object_artifact(&emitted)
            .expect("object construction independently replays the parameter store");
        let object_function = object
            .functions()
            .iter()
            .find(|candidate| candidate.machine == function.machine)
            .expect("object retains the parameter-store function");
        assert_eq!(
            object_function.unit_write_only_primitive_stores,
            function.unit_write_only_primitive_stores
        );
        let image = omega_image_emission::emit_executable_image(&object, 3)
            .expect("replayed parameter store reaches an executable image");
        let installation = omega_image_emission::build_installation_record(
            &image,
            psi_core::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("installation retains the replayed parameter store");
        let installed_function = installation
            .functions()
            .iter()
            .find(|candidate| candidate.machine == function.machine)
            .expect("installation retains the parameter-store function");
        assert_eq!(
            installed_function.unit_write_only_primitive_stores,
            function.unit_write_only_primitive_stores
        );
        let installation_bytes = omega_image_emission::encode_installation_record(&installation)
            .expect("encode installed parameter-store custody");
        let decoded = omega_image_emission::decode_installation_record(&installation_bytes)
            .expect("decode installed parameter-store custody");
        assert_eq!(decoded, installation);
        omega_image_emission::validate_installation_record(&decoded, &image)
            .expect("installed parameter store rejoins the executable image");
    }
}

#[test]
fn parameter_sourced_write_only_store_caller_reaches_verified_terminal_execution() {
    let checked = checked(
        r#"
            data Sink {}
            machine Sink::fill(destination: &write i32, replacement: i32) {
                destination = replacement;
            }

            data Root {}
            machine Root::enter(destination: &mut i32, replacement: i32) {
                Sink::fill(&write destination, replacement);
            }
        "#,
    );
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("parameter-sourced store caller reaches verified Terminal production");
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine is retained");
    let [scalar_parameter] = root.parameters.as_slice() else {
        panic!("caller retains one scalar parameter")
    };
    let [structural_parameter] = root.structural_parameters.as_slice() else {
        panic!("caller retains one structural parameter")
    };
    let call = &root.blocks[0].operations[0];
    let psi_terminal::OperationKind::CallUnit {
        callee: call_callee,
        arguments,
        structural_arguments,
        ..
    } = &call.kind
    else {
        panic!("entry operation is the Unit call")
    };
    assert_eq!(arguments, &[scalar_parameter.id]);
    assert!(matches!(
        structural_arguments.as_slice(),
        [argument] if argument.place == structural_parameter.place
            && argument.path.is_empty()
            && argument.access == psi_terminal::StructuralAccess::WriteOnlyBorrow
    ));

    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode scalar/structural Unit caller semantics");
    assert_eq!(
        psi_terminal_codec::decode_module(&semantic).expect("decode Unit caller semantics"),
        lowered.semantic_module
    );
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode Unit caller proof bundle");
    let psi_core::ScalarType::Integer(integer) = scalar_parameter.scalar_type else {
        panic!("caller parameter is an integer")
    };
    let structural = psi_terminal_interpreter::TerminalStructuralValue {
        opaque_identity: 17,
        structural_type: structural_parameter.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let replacement = psi_terminal_interpreter::TerminalScalarValue::Integer {
        scalar_type: integer,
        value: psi_core::IntegerValue::Signed(23),
    };
    let mut handler = psi_terminal_interpreter::AcceptTerminalEffects;
    let executed = psi_terminal_interpreter::interpret_terminal_artifact_with_structural_primitive_values_measured(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
        &[replacement],
        std::slice::from_ref(&structural),
        &[psi_terminal_interpreter::TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: psi_terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: integer,
                value: psi_core::IntegerValue::Signed(1),
            },
        }],
        &mut handler,
    )
    .expect("verified Unit caller forwards its scalar into write-only storage");
    assert_eq!(
        executed.structural_primitive_values(),
        &[psi_terminal_interpreter::TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: replacement,
        }]
    );
    let abstract_plan = omega_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("scalar-bearing Unit call reaches target-neutral Omega");
    let abstract_root = abstract_plan
        .functions
        .iter()
        .find(|function| function.machine == abstract_plan.entry)
        .expect("abstract caller is retained");
    let omega_abstract_operations::AbstractOperation::CallUnit {
        arguments: abstract_arguments,
        ..
    } = &abstract_root.operations[0]
    else {
        panic!("abstract caller retains one ordinary Unit call")
    };
    assert_eq!(abstract_arguments, &[scalar_parameter.id]);
    for native_target in [
        omega_target::NativeTarget::linux_x64(),
        omega_target::NativeTarget::linux_arm64(),
    ] {
        let target_plan =
            omega_abstract_operations_to_target_operations::lower_to_target_operations(
                &abstract_plan,
                native_target,
            )
            .expect("scalar-bearing Unit call reaches Target IR");
        let target_root = target_plan
            .functions
            .iter()
            .find(|function| function.machine == target_plan.entry)
            .expect("target caller is retained");
        let omega_target_operations::TargetOperation::UnitBody(target_body) =
            &target_root.operation
        else {
            panic!("target caller remains a Unit body")
        };
        let omega_target_operations::TargetUnitOperation::Call {
            call_plan,
            scalar_arguments,
            ..
        } = &target_body.operations[0]
        else {
            panic!("target caller retains one ordinary Unit call")
        };
        assert_eq!(call_plan.parameters.len(), 2);
        assert!(matches!(
            scalar_arguments.as_slice(),
            [omega_target_operations::TargetUnitScalarCallArgument {
                parameter_index: 0,
                source: omega_target_operations::TargetUnitScalarArgumentSource::Parameter {
                    parameter_index: 0,
                    source_value,
                    scalar_type,
                },
                placement,
            }] if *source_value == scalar_parameter.id
                && *scalar_type == psi_core::ScalarType::Integer(integer)
                && placement == &call_plan.parameters[0]
        ));
        let mut corrupted_target = target_plan.clone();
        let corrupted_entry = corrupted_target.entry;
        let omega_target_operations::TargetOperation::UnitBody(corrupted_body) =
            &mut corrupted_target
                .functions
                .iter_mut()
                .find(|function| function.machine == corrupted_entry)
                .expect("corrupted target caller is retained")
                .operation
        else {
            unreachable!()
        };
        let omega_target_operations::TargetUnitOperation::Call {
            scalar_arguments, ..
        } = &mut corrupted_body.operations[0]
        else {
            unreachable!()
        };
        let omega_target_operations::TargetUnitScalarArgumentSource::Parameter {
            source_value, ..
        } = &mut scalar_arguments[0].source
        else {
            unreachable!()
        };
        *source_value = psi_core::ValueId::new(9_999_999).unwrap();
        assert!(matches!(
            omega_target_operations_to_assigned_target_operations::assign_registers(
                &corrupted_target
            ),
            Err(
                omega_target_operations_to_assigned_target_operations::AssignmentError::UnitCallCustodyMismatch {
                    machine,
                    operation,
                }
            ) if machine == abstract_plan.entry && operation == call.id
        ));
        let assigned_plan =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                .expect("scalar-bearing Unit call reaches assigned Target IR");
        let assigned_root = assigned_plan
            .functions
            .iter()
            .find(|function| function.machine == assigned_plan.entry)
            .expect("assigned caller is retained");
        let omega_assigned_target_operations::AssignedOperation::UnitBody(assigned_body) =
            &assigned_root.operation
        else {
            panic!("assigned caller remains a Unit body")
        };
        let omega_assigned_target_operations::AssignedUnitOperation::Call {
            call_plan: assigned_call_plan,
            scalar_arguments: assigned_scalar_arguments,
            ..
        } = &assigned_body.operations[0]
        else {
            panic!("assigned caller retains one ordinary Unit call")
        };
        assert_eq!(assigned_call_plan, call_plan);
        assert!(matches!(
            assigned_scalar_arguments.as_slice(),
            [omega_assigned_target_operations::AssignedUnitScalarCallArgument {
                parameter_index: 0,
                source: omega_assigned_target_operations::AssignedUnitScalarArgumentSource::Parameter {
                    parameter_index: 0,
                    source_value,
                    scalar_type,
                    ..
                },
                ..
            }] if *source_value == scalar_parameter.id
                && *scalar_type == psi_core::ScalarType::Integer(integer)
        ));
        let emitted = omega_machine_emission::emit_machine_code(&assigned_plan)
            .expect("scalar-bearing Unit call reaches machine-code custody");
        let emitted_root = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("emitted caller is retained");
        let [emitted_call] = emitted_root.internal_unit_calls.as_slice() else {
            panic!("emitted caller retains one internal Unit call")
        };
        assert_eq!(emitted_call.target, *call_callee);
        assert!(matches!(
                emitted_call.scalar_arguments.as_slice(),
                [omega_machine_code::InternalUnitScalarCallArgumentRecord {
                    parameter_index: 0,
                    source: omega_machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                        parameter_index: 0,
                        source_value,
                        scalar_type,
                        ..
                    },
                    destination,
                    ..
            }] if *source_value == scalar_parameter.id
                && *scalar_type == psi_core::ScalarType::Integer(integer)
                && destination == &call_plan.parameters[0]
        ));
        let mut corrupted_emitted = emitted.clone();
        let source = &mut corrupted_emitted
            .functions
            .iter_mut()
            .find(|function| function.machine == emitted.entry)
            .unwrap()
            .internal_unit_calls[0]
            .scalar_arguments[0]
            .source;
        let omega_machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
            location, ..
        } = source
        else {
            unreachable!()
        };
        *location = match native_target.architecture {
            omega_target::Architecture::X86_64 => {
                omega_machine_code::UnitScalarParameterLocationRecord::Register(
                    omega_target_operations::MachineRegister::X86Rax,
                )
            }
            omega_target::Architecture::Aarch64 => {
                omega_machine_code::UnitScalarParameterLocationRecord::Register(
                    omega_target_operations::MachineRegister::Aarch64X(1),
                )
            }
        };
        assert_eq!(
            omega_image_emission::build_object_artifact(&corrupted_emitted),
            Err(
                omega_image_emission::ObjectError::InvalidInternalUnitCallEvidence(
                    abstract_plan.entry
                )
            )
        );
        let object = omega_image_emission::build_object_artifact(&emitted)
            .expect("object construction replays the scalar-bearing Unit call");
        let image = omega_image_emission::emit_executable_image(&object, 3)
            .expect("scalar-bearing Unit call reaches an executable image");
        let installation = omega_image_emission::build_installation_record(
            &image,
            psi_core::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("installation retains scalar-bearing Unit call custody");
        let installation_bytes = omega_image_emission::encode_installation_record(&installation)
            .expect("encode scalar-bearing Unit call custody");
        let decoded = omega_image_emission::decode_installation_record(&installation_bytes)
            .expect("decode scalar-bearing Unit call custody");
        assert_eq!(decoded, installation);
        omega_image_emission::validate_installation_record(&decoded, &image)
            .expect("installation independently replays scalar-bearing Unit call custody");
    }

    let mut missing_argument = lowered.semantic_module.clone();
    let entry = missing_argument.entry;
    let missing_call = missing_argument
        .machines
        .iter_mut()
        .find(|machine| machine.id == entry)
        .expect("mutated caller remains present")
        .blocks[0]
        .operations
        .first_mut()
        .expect("mutated caller retains its call");
    let psi_terminal::OperationKind::CallUnit { arguments, .. } = &mut missing_call.kind else {
        panic!("mutated entry operation remains the Unit call")
    };
    arguments.clear();
    let missing_operation = missing_call.id;
    assert!(matches!(
        psi_terminal_verifier::validate_module(&missing_argument),
        Err(
            psi_terminal_verifier::ModuleError::CallArgumentArityMismatch {
                operation,
                expected: 1,
                actual: 0,
            }
        ) if operation == missing_operation
    ));
}
