//! Stack-carried Boolean forwarding controls.

use super::*;

#[test]
fn stack_carried_boolean_reaches_a_write_only_store_and_canonical_installation() {
    let checked = checked(
        r#"
            data Sink {}
            machine Sink::fill(
                destination: &write bool,
                p0: bool, p1: bool, p2: bool, p3: bool, p4: bool,
                p5: bool, p6: bool, p7: bool, p8: bool
            ) {
                destination = p8;
            }

            data Root {}
            machine Root::enter(
                destination: &mut bool,
                p0: bool, p1: bool, p2: bool, p3: bool, p4: bool,
                p5: bool, p6: bool, p7: bool, p8: bool
            ) {
                Sink::fill(&write destination, p0, p1, p2, p3, p4, p5, p6, p7, p8);
            }
        "#,
    );
    let lowered = checked_trees_to_terminal_psi::lower_machine(&checked, "Root::enter")
        .expect("Boolean caller reaches verified Terminal production");
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("Boolean caller is retained");
    assert_eq!(root.parameters.len(), 9);
    let replacement_parameter = &root.parameters[8];
    let [structural_parameter] = root.structural_parameters.as_slice() else {
        panic!("Boolean caller retains one structural parameter")
    };
    assert_eq!(
        replacement_parameter.scalar_type,
        semantic_vocabulary::ScalarType::Boolean
    );
    let semantic = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode Boolean caller semantics");
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode Boolean caller proof");
    let replacement = terminal_interpreter::TerminalScalarValue::Boolean(false);
    let mut handler = terminal_interpreter::AcceptTerminalEffects;
    let executed = terminal_interpreter::interpret_terminal_artifact_with_structural_primitive_values_measured(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
        &[
            terminal_interpreter::TerminalScalarValue::Boolean(true),
            terminal_interpreter::TerminalScalarValue::Boolean(true),
            terminal_interpreter::TerminalScalarValue::Boolean(true),
            terminal_interpreter::TerminalScalarValue::Boolean(true),
            terminal_interpreter::TerminalScalarValue::Boolean(true),
            terminal_interpreter::TerminalScalarValue::Boolean(true),
            terminal_interpreter::TerminalScalarValue::Boolean(true),
            terminal_interpreter::TerminalScalarValue::Boolean(true),
            replacement,
        ],
        &[terminal_interpreter::TerminalStructuralValue {
            opaque_identity: 23,
            structural_type: structural_parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
        &[terminal_interpreter::TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: terminal_interpreter::TerminalScalarValue::Boolean(true),
        }],
        &mut handler,
    )
    .expect("Boolean caller forwards replacement into write-only storage");
    assert_eq!(
        executed.structural_primitive_values(),
        &[terminal_interpreter::TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: replacement,
        }]
    );
    let abstract_plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("Boolean caller reaches Abstract operations");

    for native_target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            native_target,
        )
        .expect("Boolean caller reaches Target IR");
        let target_root = target
            .functions
            .iter()
            .find(|function| function.machine == target.entry)
            .expect("Target caller is retained");
        let target_operations::TargetOperation::UnitBody(body) = &target_root.operation else {
            panic!("Target caller remains a Unit body")
        };
        let target_operations::TargetUnitOperation::Call {
            scalar_arguments, ..
        } = &body.operations[0]
        else {
            panic!("Target caller retains its Unit call")
        };
        assert_eq!(scalar_arguments.len(), 9);
        for (index, (argument, parameter)) in
            scalar_arguments.iter().zip(&root.parameters).enumerate()
        {
            assert!(matches!(
                argument.source,
                target_operations::TargetUnitScalarArgumentSource::Parameter {
                    parameter_index,
                    source_value,
                    scalar_type: semantic_vocabulary::ScalarType::Boolean,
                } if usize::try_from(parameter_index) == Ok(index) && source_value == parameter.id
            ));
        }
        let assigned = target_operations_to_assigned_target_operations::assign_registers(&target)
            .expect("Boolean caller reaches physical assignment");
        let emitted = machine_emission::emit_machine_code(&assigned)
            .expect("Boolean caller reaches machine emission");
        let emitted_root = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("emitted Boolean caller is retained");
        let [call] = emitted_root.internal_unit_calls.as_slice() else {
            panic!("emitted Boolean caller retains one internal Unit call")
        };
        assert_eq!(call.scalar_arguments.len(), 9);
        assert!(call.scalar_arguments.iter().all(|argument| {
            matches!(
                argument.source,
                machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                    scalar_type: semantic_vocabulary::ScalarType::Boolean,
                    ..
                }
            )
        }));
        assert!(matches!(
            call.scalar_arguments[8].source,
            machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                parameter_index: 8,
                location: machine_code::UnitScalarParameterLocationRecord::IncomingStack { .. },
                ..
            }
        ));
        assert_ne!(call.scalar_arguments[8].byte_count, 0);
        let emitted_sink = emitted
            .functions
            .iter()
            .find(|function| function.machine == call.target)
            .expect("emitted Boolean callee is retained");
        assert!(matches!(
            emitted_sink.unit_write_only_primitive_stores[0].source,
            machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                parameter_index: 8,
                scalar_type: semantic_vocabulary::ScalarType::Boolean,
                location: machine_code::UnitScalarParameterLocationRecord::IncomingStack { .. },
                ..
            }
        ));
        let mut corrupted = emitted.clone();
        let machine_code::InternalUnitScalarArgumentSourceRecord::Parameter { scalar_type, .. } =
            &mut corrupted
                .functions
                .iter_mut()
                .find(|function| function.machine == emitted.entry)
                .unwrap()
                .internal_unit_calls[0]
                .scalar_arguments[8]
                .source
        else {
            unreachable!()
        };
        *scalar_type = semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .unwrap(),
        );
        assert_eq!(
            image_emission::build_object_artifact(&corrupted),
            Err(image_emission::ObjectError::InvalidInternalUnitCallEvidence(emitted.entry))
        );
        let mut changed_call_stack_offset = emitted.clone();
        let machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
            location: machine_code::UnitScalarParameterLocationRecord::IncomingStack { byte_offset },
            ..
        } = &mut changed_call_stack_offset
            .functions
            .iter_mut()
            .find(|function| function.machine == emitted.entry)
            .unwrap()
            .internal_unit_calls[0]
            .scalar_arguments[8]
            .source
        else {
            unreachable!()
        };
        *byte_offset += 8;
        assert_eq!(
            image_emission::build_object_artifact(&changed_call_stack_offset),
            Err(image_emission::ObjectError::InvalidInternalUnitCallEvidence(emitted.entry))
        );
        let sink_machine = emitted_sink.machine;
        let mut changed_store_stack_offset = emitted.clone();
        let machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
            location: machine_code::UnitScalarParameterLocationRecord::IncomingStack { byte_offset },
            ..
        } = &mut changed_store_stack_offset
            .functions
            .iter_mut()
            .find(|function| function.machine == sink_machine)
            .unwrap()
            .unit_write_only_primitive_stores[0]
            .source
        else {
            unreachable!()
        };
        *byte_offset += 8;
        assert_eq!(
            image_emission::build_object_artifact(&changed_store_stack_offset),
            Err(
                image_emission::ObjectError::InvalidUnitWriteOnlyPrimitiveStoreEvidence(
                    sink_machine
                )
            )
        );
        let object = image_emission::build_object_artifact(&emitted)
            .expect("object replay accepts Boolean caller custody");
        let image = image_emission::emit_executable_image(&object, 3)
            .expect("Boolean caller reaches an executable image");
        let installation = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("installation retains Boolean caller custody");
        let bytes = image_emission::encode_installation_record(&installation)
            .expect("encode installed Boolean caller");
        let decoded = image_emission::decode_installation_record(&bytes)
            .expect("decode installed Boolean caller");
        assert_eq!(decoded, installation);
        image_emission::validate_installation_record(&decoded, &image)
            .expect("installation replays Boolean caller custody");
        let installed_store_bytes = installation
            .functions()
            .iter()
            .find(|function| function.machine == sink_machine)
            .unwrap()
            .unit_write_only_primitive_stores[0]
            .bytes
            .clone();
        let mut corrupted_installation = bytes;
        let encoded_store = corrupted_installation
            .windows(installed_store_bytes.len())
            .rposition(|window| window == installed_store_bytes)
            .expect("stack-sourced store bytes occur in the canonical installation record");
        corrupted_installation[encoded_store] ^= 1;
        assert!(matches!(
            image_emission::decode_installation_record(&corrupted_installation),
            Err(
                image_emission::InstallationError::InvalidUnitWriteOnlyPrimitiveStore(
                    machine
                )
            ) if machine == sink_machine
        ));
    }
}
