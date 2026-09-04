//! Focused write-only lowering and installation controls.

use super::*;

#[test]
fn boolean_parameter_sourced_write_only_store_reaches_canonical_installation() {
    let checked = checked(
        r#"
            data Sink {}
            machine Sink::fill(destination: &write bool, replacement: bool) {
                destination = replacement;
            }
        "#,
    );
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Sink::fill")
        .expect("Boolean parameter store reaches Terminal production");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("Boolean store machine is retained");
    let [scalar_parameter] = entry.parameters.as_slice() else {
        panic!("Boolean store retains one scalar parameter")
    };
    let [structural_parameter] = entry.structural_parameters.as_slice() else {
        panic!("Boolean store retains one structural parameter")
    };
    assert_eq!(scalar_parameter.scalar_type, psi_core::ScalarType::Boolean);
    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode Boolean parameter store semantics");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode Boolean parameter store proof bundle");
    let replacement = psi_terminal_interpreter::TerminalScalarValue::Boolean(false);
    let mut handler = psi_terminal_interpreter::AcceptTerminalEffects;
    let executed = psi_terminal_interpreter::interpret_terminal_artifact_with_structural_primitive_values_measured(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
        &[replacement],
        &[psi_terminal_interpreter::TerminalStructuralValue {
            opaque_identity: 19,
            structural_type: structural_parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
        &[psi_terminal_interpreter::TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: psi_terminal_interpreter::TerminalScalarValue::Boolean(true),
        }],
        &mut handler,
    )
    .expect("Boolean parameter replaces write-only backing without observation");
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
    .expect("Boolean parameter store reaches Abstract operations");

    for native_target in [
        omega_target::NativeTarget::linux_x64(),
        omega_target::NativeTarget::linux_arm64(),
    ] {
        let target = omega_abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            native_target,
        )
        .expect("Boolean parameter store reaches Target IR");
        let target_source = target
            .functions
            .iter()
            .find_map(|function| match &function.operation {
                omega_target_operations::TargetOperation::UnitBody(body) => body
                    .operations
                    .iter()
                    .find_map(|operation| match operation {
                        omega_target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore {
                            source,
                            ..
                        } => Some(source),
                        _ => None,
                    }),
                _ => None,
            })
            .expect("Target IR retains the Boolean runtime source");
        assert!(matches!(
            target_source,
            omega_target_operations::TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
                parameter_index: 0,
                source_value,
                scalar_type: psi_core::ScalarType::Boolean,
            } if *source_value == scalar_parameter.id
        ));
        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target)
                .expect("Boolean parameter store reaches physical assignment");
        let emitted = omega_machine_emission::emit_machine_code(&assigned)
            .expect("Boolean parameter store reaches exact machine emission");
        let function = emitted
            .functions
            .iter()
            .find(|function| function.unit_write_only_primitive_stores.len() == 1)
            .expect("one machine owns the Boolean parameter store");
        let store = &function.unit_write_only_primitive_stores[0];
        assert!(matches!(
            store.source,
            omega_machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                parameter_index: 0,
                source_value,
                scalar_type: psi_core::ScalarType::Boolean,
                location: omega_machine_code::UnitScalarParameterLocationRecord::Register(register),
            } if source_value == scalar_parameter.id
                && register.architecture() == native_target.architecture
        ));
        match native_target.architecture {
            omega_target::Architecture::X86_64 => {
                assert!(store.bytes.ends_with(&[0x41, 0x88, 0x3a]));
            }
            omega_target::Architecture::Aarch64 => {
                assert_eq!(
                    store.bytes.last_chunk::<4>(),
                    Some(&0x3900_0220_u32.to_le_bytes())
                );
            }
        }
        let mut corrupted = emitted.clone();
        let omega_machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
            scalar_type,
            ..
        } = &mut corrupted
            .functions
            .iter_mut()
            .find(|candidate| candidate.machine == function.machine)
            .unwrap()
            .unit_write_only_primitive_stores[0]
            .source
        else {
            unreachable!()
        };
        *scalar_type = psi_core::ScalarType::Integer(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
        );
        assert!(matches!(
            omega_image_emission::build_object_artifact(&corrupted),
            Err(
                omega_image_emission::ObjectError::InvalidUnitWriteOnlyPrimitiveStoreEvidence(
                    machine
                )
            ) if machine == function.machine
        ));
        let object = omega_image_emission::build_object_artifact(&emitted)
            .expect("object replay accepts Boolean parameter store custody");
        let image = omega_image_emission::emit_executable_image(&object, 3)
            .expect("Boolean parameter store reaches an executable image");
        let installation = omega_image_emission::build_installation_record(
            &image,
            psi_core::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("installation retains Boolean parameter store custody");
        let bytes = omega_image_emission::encode_installation_record(&installation)
            .expect("encode installed Boolean parameter store");
        let decoded = omega_image_emission::decode_installation_record(&bytes)
            .expect("decode installed Boolean parameter store");
        assert_eq!(decoded, installation);
        omega_image_emission::validate_installation_record(&decoded, &image)
            .expect("installation replays Boolean parameter store custody");
    }
}

#[test]
fn multiple_boolean_parameters_reach_a_write_only_store_and_canonical_installation() {
    let checked = checked(
        r#"
            data Sink {}
            machine Sink::fill(destination: &write bool, ignored: bool, replacement: bool) {
                destination = replacement;
            }

            data Root {}
            machine Root::enter(destination: &mut bool, ignored: bool, replacement: bool) {
                Sink::fill(&write destination, ignored, replacement);
            }
        "#,
    );
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("Boolean caller reaches verified Terminal production");
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("Boolean caller is retained");
    let [ignored_parameter, replacement_parameter] = root.parameters.as_slice() else {
        panic!("Boolean caller retains both scalar parameters")
    };
    let [structural_parameter] = root.structural_parameters.as_slice() else {
        panic!("Boolean caller retains one structural parameter")
    };
    assert_eq!(ignored_parameter.scalar_type, psi_core::ScalarType::Boolean);
    assert_eq!(
        replacement_parameter.scalar_type,
        psi_core::ScalarType::Boolean
    );
    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode Boolean caller semantics");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode Boolean caller proof");
    let replacement = psi_terminal_interpreter::TerminalScalarValue::Boolean(false);
    let mut handler = psi_terminal_interpreter::AcceptTerminalEffects;
    let executed = psi_terminal_interpreter::interpret_terminal_artifact_with_structural_primitive_values_measured(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
        &[
            psi_terminal_interpreter::TerminalScalarValue::Boolean(true),
            replacement,
        ],
        &[psi_terminal_interpreter::TerminalStructuralValue {
            opaque_identity: 23,
            structural_type: structural_parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
        &[psi_terminal_interpreter::TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: psi_terminal_interpreter::TerminalScalarValue::Boolean(true),
        }],
        &mut handler,
    )
    .expect("Boolean caller forwards replacement into write-only storage");
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
    .expect("Boolean caller reaches Abstract operations");

    for native_target in [
        omega_target::NativeTarget::linux_x64(),
        omega_target::NativeTarget::linux_arm64(),
    ] {
        let target = omega_abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            native_target,
        )
        .expect("Boolean caller reaches Target IR");
        let target_root = target
            .functions
            .iter()
            .find(|function| function.machine == target.entry)
            .expect("Target caller is retained");
        let omega_target_operations::TargetOperation::UnitBody(body) = &target_root.operation
        else {
            panic!("Target caller remains a Unit body")
        };
        let omega_target_operations::TargetUnitOperation::Call {
            scalar_arguments, ..
        } = &body.operations[0]
        else {
            panic!("Target caller retains its Unit call")
        };
        assert_eq!(scalar_arguments.len(), 2);
        for (index, (argument, parameter)) in scalar_arguments
            .iter()
            .zip([ignored_parameter, replacement_parameter])
            .enumerate()
        {
            assert!(matches!(
                argument.source,
                omega_target_operations::TargetUnitScalarArgumentSource::Parameter {
                    parameter_index,
                    source_value,
                    scalar_type: psi_core::ScalarType::Boolean,
                } if usize::try_from(parameter_index) == Ok(index) && source_value == parameter.id
            ));
        }
        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target)
                .expect("Boolean caller reaches physical assignment");
        let emitted = omega_machine_emission::emit_machine_code(&assigned)
            .expect("Boolean caller reaches machine emission");
        let emitted_root = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("emitted Boolean caller is retained");
        let [call] = emitted_root.internal_unit_calls.as_slice() else {
            panic!("emitted Boolean caller retains one internal Unit call")
        };
        assert_eq!(call.scalar_arguments.len(), 2);
        assert!(call.scalar_arguments.iter().all(|argument| {
            matches!(
                argument.source,
                omega_machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                    scalar_type: psi_core::ScalarType::Boolean,
                    ..
                }
            ) && argument.byte_count == 0
        }));
        let emitted_sink = emitted
            .functions
            .iter()
            .find(|function| function.machine == call.target)
            .expect("emitted Boolean callee is retained");
        assert!(matches!(
            emitted_sink.unit_write_only_primitive_stores[0].source,
            omega_machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                parameter_index: 1,
                scalar_type: psi_core::ScalarType::Boolean,
                ..
            }
        ));
        let mut corrupted = emitted.clone();
        let omega_machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
            scalar_type,
            ..
        } = &mut corrupted
            .functions
            .iter_mut()
            .find(|function| function.machine == emitted.entry)
            .unwrap()
            .internal_unit_calls[0]
            .scalar_arguments[1]
            .source
        else {
            unreachable!()
        };
        *scalar_type = psi_core::ScalarType::Integer(
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
        );
        assert_eq!(
            omega_image_emission::build_object_artifact(&corrupted),
            Err(omega_image_emission::ObjectError::InvalidInternalUnitCallEvidence(emitted.entry))
        );
        let object = omega_image_emission::build_object_artifact(&emitted)
            .expect("object replay accepts Boolean caller custody");
        let image = omega_image_emission::emit_executable_image(&object, 3)
            .expect("Boolean caller reaches an executable image");
        let installation = omega_image_emission::build_installation_record(
            &image,
            psi_core::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("installation retains Boolean caller custody");
        let bytes = omega_image_emission::encode_installation_record(&installation)
            .expect("encode installed Boolean caller");
        let decoded = omega_image_emission::decode_installation_record(&bytes)
            .expect("decode installed Boolean caller");
        assert_eq!(decoded, installation);
        omega_image_emission::validate_installation_record(&decoded, &image)
            .expect("installation replays Boolean caller custody");
    }
}
