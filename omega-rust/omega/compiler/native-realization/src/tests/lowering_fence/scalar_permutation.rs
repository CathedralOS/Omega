//! Scalar parameter permutation controls.

use super::*;

#[test]
fn scalar_parameter_permutation_reaches_canonical_installation() {
    let checked = checked(
        r#"
            data Sink {}
            machine Sink::fill(
                destination: &write bool,
                q0: bool, q1: bool, q2: bool, q3: bool, q4: bool,
                q5: bool, q6: bool, q7: bool, q8: bool
            ) {
                destination = q0;
            }

            data Root {}
            machine Root::enter(
                destination: &mut bool,
                p0: bool, p1: bool, p2: bool, p3: bool, p4: bool,
                p5: bool, p6: bool, p7: bool, p8: bool
            ) {
                Sink::fill(&write destination, p1, p0, p8, p2, p3, p4, p5, p6, p7);
            }
        "#,
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("permuted Boolean caller reaches verified Terminal production");
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("permuted Boolean caller is retained");
    let [structural_parameter] = root.structural_parameters.as_slice() else {
        panic!("permuted Boolean caller retains one structural parameter")
    };
    let semantic = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode permuted Boolean caller semantics");
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode permuted Boolean caller proof");
    let mut handler = terminal_interpreter::AcceptTerminalEffects;
    let executed = terminal_interpreter::interpret_terminal_artifact_with_structural_primitive_values_measured(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
        &[
            terminal_interpreter::TerminalScalarValue::Boolean(true),
            terminal_interpreter::TerminalScalarValue::Boolean(false),
            terminal_interpreter::TerminalScalarValue::Boolean(true),
            terminal_interpreter::TerminalScalarValue::Boolean(true),
            terminal_interpreter::TerminalScalarValue::Boolean(true),
            terminal_interpreter::TerminalScalarValue::Boolean(true),
            terminal_interpreter::TerminalScalarValue::Boolean(true),
            terminal_interpreter::TerminalScalarValue::Boolean(true),
            terminal_interpreter::TerminalScalarValue::Boolean(true),
        ],
        &[terminal_interpreter::TerminalStructuralValue {
            opaque_identity: 29,
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
    .expect("permuted Boolean caller executes through Terminal");
    assert_eq!(
        executed.structural_primitive_values(),
        &[terminal_interpreter::TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: terminal_interpreter::TerminalScalarValue::Boolean(false),
        }]
    );
    let abstract_plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("permuted Boolean caller reaches Abstract operations");

    for native_target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            native_target,
        )
        .expect("permuted Boolean caller reaches Target IR");
        let assigned = target_operations_to_assigned_target_operations::assign_registers(&target)
            .expect("permuted Boolean caller reaches physical assignment");
        let emitted = machine_emission::emit_machine_code(&assigned)
            .expect("permuted Boolean caller reaches machine emission");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("emitted permuted caller is retained");
        let [call] = caller.internal_unit_calls.as_slice() else {
            panic!("emitted permuted caller retains one Unit call")
        };
        assert_eq!(call.scalar_arguments.len(), 9);
        assert!(
            call.scalar_arguments
                .iter()
                .all(|argument| argument.byte_count != 0)
        );
        assert_eq!(
            call.scalar_arguments
                .iter()
                .map(|argument| match argument.source {
                    machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                        parameter_index,
                        ..
                    } => parameter_index,
                    _ => panic!("permutation retains only parameter sources"),
                })
                .collect::<Vec<_>>(),
            vec![1, 0, 8, 2, 3, 4, 5, 6, 7]
        );
        let mut corrupted = emitted.clone();
        let machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index, ..
        } = &mut corrupted
            .functions
            .iter_mut()
            .find(|function| function.machine == emitted.entry)
            .unwrap()
            .internal_unit_calls[0]
            .scalar_arguments[0]
            .source
        else {
            unreachable!()
        };
        *parameter_index = 2;
        assert_eq!(
            image_emission::build_object_artifact(&corrupted),
            Err(image_emission::ObjectError::InvalidInternalUnitCallEvidence(emitted.entry))
        );
        let mut changed_snapshot_bytes = emitted.clone();
        let snapshot_offset = call.scalar_arguments[0].code_offset;
        changed_snapshot_bytes
            .functions
            .iter_mut()
            .find(|function| function.machine == emitted.entry)
            .unwrap()
            .bytes[snapshot_offset] ^= 1;
        assert_eq!(
            image_emission::build_object_artifact(&changed_snapshot_bytes),
            Err(image_emission::ObjectError::InvalidInternalUnitCallEvidence(emitted.entry))
        );
        let object = image_emission::build_object_artifact(&emitted)
            .expect("object replay accepts cycle-safe scalar permutation custody");
        let image = image_emission::emit_executable_image(&object, 3)
            .expect("permuted Boolean caller reaches an executable image");
        let installation = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("installation retains permuted scalar custody");
        let bytes = image_emission::encode_installation_record(&installation)
            .expect("encode permuted scalar installation");
        let decoded = image_emission::decode_installation_record(&bytes)
            .expect("decode permuted scalar installation");
        assert_eq!(decoded, installation);
        image_emission::validate_installation_record(&decoded, &image)
            .expect("installation replays cycle-safe scalar permutation custody");
    }
}
