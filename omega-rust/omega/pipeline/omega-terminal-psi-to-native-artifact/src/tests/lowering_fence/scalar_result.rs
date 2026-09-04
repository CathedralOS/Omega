//! Runtime scalar-result forwarding controls.

use super::*;
use std::sync::Arc;

use crate::tests::fixtures::checked_source::checked_with_sole_selected_provider;

fn checked_with_selected_integer_operator_store() -> Arc<psi_checked_trees::CheckedTrees> {
    let source = r#"
        data CheckedMath {}
        boundary operator CheckedMath::offset_zero(value: i32) -> i32
        requires value == value
        ensures result == value + 0 && value == value;

        data CheckedMathProvider {}
        machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
        satisfies CheckedMath::offset_zero
        requires input == input
        ensures result == input + 0 && input == input
        {
            transition { _ -> (input + 0) }
        }

        data Root {}
        machine Root::enter(destination: &write i32) {
            let replacement: i32 = CheckedMath::offset_zero(23);
            destination = replacement;
        }
    "#;
    checked_with_sole_selected_provider(source)
}

#[test]
fn scalar_result_home_reaches_a_write_only_store_and_canonical_installation() {
    let checked = checked(
        r#"
            data Scalar {}
            machine Scalar::identity(value: i32) -> i32
            requires value == value
            ensures result == value
            {
                transition { _ -> value }
            }

            data Sink {}
            machine Sink::fill(destination: &write i32, replacement: i32) {
                destination = replacement;
            }

            data Root {}
            machine Root::enter(destination: &mut i32) {
                let replacement: i32 = Scalar::identity(23);
                Sink::fill(&write destination, replacement);
            }
        "#,
    );
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("runtime scalar result caller reaches verified Terminal production");
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("runtime scalar result caller is retained");
    let [structural_parameter] = root.structural_parameters.as_slice() else {
        panic!("runtime scalar result caller retains one structural parameter")
    };
    let scalar_call = root.blocks[0]
        .operations
        .iter()
        .find(|operation| matches!(operation.kind, psi_terminal::OperationKind::Call { .. }))
        .expect("runtime scalar result caller retains one scalar call");
    let unit_call = root.blocks[0]
        .operations
        .iter()
        .find(|operation| matches!(operation.kind, psi_terminal::OperationKind::CallUnit { .. }))
        .expect("runtime scalar result caller retains one Unit call");
    assert!(matches!(
        scalar_call.kind,
        psi_terminal::OperationKind::Call { .. }
    ));
    let psi_terminal::OperationResult::Scalar(result) = &scalar_call.result else {
        panic!("first operation produces the runtime scalar")
    };
    let psi_terminal::OperationKind::CallUnit { arguments, .. } = &unit_call.kind else {
        panic!("second operation forwards the runtime scalar")
    };
    assert_eq!(arguments, &[result.id]);
    let psi_core::ScalarType::Integer(integer) = result.scalar_type else {
        panic!("runtime scalar result is an integer")
    };

    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode runtime scalar result caller semantics");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode runtime scalar result caller proof");
    let mut handler = psi_terminal_interpreter::AcceptTerminalEffects;
    let executed = psi_terminal_interpreter::interpret_terminal_artifact_with_structural_primitive_values_measured(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
        &[],
        &[psi_terminal_interpreter::TerminalStructuralValue {
            opaque_identity: 31,
            structural_type: structural_parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
        &[psi_terminal_interpreter::TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: psi_terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: integer,
                value: psi_core::IntegerValue::Signed(1),
            },
        }],
        &mut handler,
    )
    .expect("runtime scalar result reaches write-only storage in reference execution");
    assert_eq!(
        executed.structural_primitive_values(),
        &[psi_terminal_interpreter::TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: psi_terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: integer,
                value: psi_core::IntegerValue::Signed(23),
            },
        }]
    );

    let abstract_plan = omega_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("runtime scalar result caller reaches Abstract operations");
    for native_target in [
        omega_target::NativeTarget::linux_x64(),
        omega_target::NativeTarget::linux_arm64(),
    ] {
        let target = omega_abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            native_target,
        )
        .expect("runtime scalar result caller reaches Target IR");
        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target)
                .expect("runtime scalar result caller reaches physical assignment");
        let emitted = omega_machine_emission::emit_machine_code(&assigned)
            .expect("runtime scalar result caller reaches machine emission");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("emitted runtime scalar result caller is retained");
        let [producer] = caller.internal_unit_scalar_calls.as_slice() else {
            panic!("caller retains one scalar result producer")
        };
        let [consumer] = caller.internal_unit_calls.as_slice() else {
            panic!("caller retains one mixed Unit consumer")
        };
        let [argument] = consumer.scalar_arguments.as_slice() else {
            panic!("mixed Unit consumer retains one scalar argument")
        };
        assert_eq!(
            argument.source,
            omega_machine_code::InternalUnitScalarArgumentSourceRecord::Home(producer.result.home)
        );
        assert_ne!(argument.byte_count, 0);

        let mut changed_home = emitted.clone();
        let omega_machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) =
            &mut changed_home
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
        home.byte_offset = home.byte_offset.checked_add(8).unwrap();
        assert_eq!(
            omega_image_emission::build_object_artifact(&changed_home),
            Err(omega_image_emission::ObjectError::InvalidInternalUnitCallEvidence(emitted.entry))
        );
        let mut changed_bytes = emitted.clone();
        changed_bytes
            .functions
            .iter_mut()
            .find(|function| function.machine == emitted.entry)
            .unwrap()
            .bytes[argument.code_offset] ^= 1;
        assert_eq!(
            omega_image_emission::build_object_artifact(&changed_bytes),
            Err(omega_image_emission::ObjectError::InvalidInternalUnitCallEvidence(emitted.entry))
        );

        let object = omega_image_emission::build_object_artifact(&emitted)
            .expect("object replay accepts runtime scalar result forwarding");
        let image = omega_image_emission::emit_executable_image(&object, 3)
            .expect("runtime scalar result caller reaches an executable image");
        let installation = omega_image_emission::build_installation_record(
            &image,
            psi_core::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("installation retains runtime scalar result forwarding");
        let bytes = omega_image_emission::encode_installation_record(&installation)
            .expect("encode runtime scalar result installation");
        let decoded = omega_image_emission::decode_installation_record(&bytes)
            .expect("decode runtime scalar result installation");
        assert_eq!(decoded, installation);
        omega_image_emission::validate_installation_record(&decoded, &image)
            .expect("installation replays runtime scalar result forwarding");
    }
}

fn assert_scalar_result_home_directly_reaches_a_write_only_store_and_canonical_installation(
    checked: &psi_checked_trees::CheckedTrees,
) {
    let lowered = psi_checked_trees_to_terminal::lower_machine(checked, "Root::enter")
        .expect("direct scalar-result store reaches verified Terminal production");
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("direct scalar-result store owner is retained");
    let [structural_parameter] = root.structural_parameters.as_slice() else {
        panic!("direct scalar-result store owner retains one structural parameter")
    };
    let [constant, scalar_call, store] = root.blocks[0].operations.as_slice() else {
        panic!("direct scalar-result store retains its exact three-operation lane")
    };
    assert!(matches!(
        constant.kind,
        psi_terminal::OperationKind::IntegerConstant { .. }
    ));
    let psi_terminal::OperationResult::Scalar(result) = scalar_call.result else {
        panic!("scalar call produces the runtime scalar")
    };
    assert!(matches!(
        scalar_call.kind,
        psi_terminal::OperationKind::Call { .. }
    ));
    assert!(matches!(
        store.kind,
        psi_terminal::OperationKind::WriteOnlyPrimitiveStore { value, .. }
            if value == result.id
    ));
    assert!(matches!(
        root.blocks[0].terminator,
        psi_terminal::Terminator::ReturnUnit { .. }
    ));
    let psi_core::ScalarType::Integer(integer) = result.scalar_type else {
        panic!("direct scalar-result store carries an integer")
    };

    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode direct scalar-result store semantics");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode direct scalar-result store proof");
    let mut handler = psi_terminal_interpreter::AcceptTerminalEffects;
    let executed = psi_terminal_interpreter::interpret_terminal_artifact_with_structural_primitive_values_measured(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
        &[],
        &[psi_terminal_interpreter::TerminalStructuralValue {
            opaque_identity: 37,
            structural_type: structural_parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
        &[psi_terminal_interpreter::TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: psi_terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: integer,
                value: psi_core::IntegerValue::Signed(1),
            },
        }],
        &mut handler,
    )
    .expect("reference execution stores the direct scalar result");
    assert_eq!(
        executed.structural_primitive_values(),
        &[psi_terminal_interpreter::TerminalStructuralPrimitiveValue {
            argument_index: 0,
            value: psi_terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: integer,
                value: psi_core::IntegerValue::Signed(23),
            },
        }]
    );

    let abstract_plan = omega_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("direct scalar-result store reaches Abstract operations");
    for native_target in [
        omega_target::NativeTarget::linux_x64(),
        omega_target::NativeTarget::linux_arm64(),
    ] {
        let target = omega_abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            native_target,
        )
        .expect("direct scalar-result store reaches Target IR");
        let target_body = target
            .functions
            .iter()
            .find(|function| function.machine == target.entry)
            .and_then(|function| match &function.operation {
                omega_target_operations::TargetOperation::UnitBody(body) => Some(body),
                _ => None,
            })
            .expect("Target caller remains a Unit body");
        let target_result_home = target_body
            .operations
            .iter()
            .find_map(|operation| match operation {
                omega_target_operations::TargetUnitOperation::ScalarCall {
                    result_home, ..
                } => Some(result_home),
                _ => None,
            })
            .expect("Target caller retains the scalar producer");
        let target_store_home = target_body
            .operations
            .iter()
            .find_map(|operation| match operation {
                omega_target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore {
                    source:
                        omega_target_operations::TargetUnitWriteOnlyPrimitiveStoreSource::Home(
                            home,
                        ),
                    ..
                } => Some(home),
                _ => None,
            })
            .expect("Target store reads the scalar result home");
        assert_eq!(target_store_home, target_result_home);

        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target)
                .expect("direct scalar-result store reaches physical assignment");
        let assigned_body = assigned
            .functions
            .iter()
            .find(|function| function.machine == assigned.entry)
            .and_then(|function| match &function.operation {
                omega_assigned_target_operations::AssignedOperation::UnitBody(body) => Some(body),
                _ => None,
            })
            .expect("assigned caller remains a Unit body");
        let assigned_result_home = assigned_body
            .operations
            .iter()
            .find_map(|operation| match operation {
                omega_assigned_target_operations::AssignedUnitOperation::ScalarCall {
                    result_home,
                    ..
                } => Some(result_home),
                _ => None,
            })
            .expect("assigned caller retains the scalar producer");
        let assigned_store_home = assigned_body
            .operations
            .iter()
            .find_map(|operation| match operation {
                omega_assigned_target_operations::AssignedUnitOperation::WriteOnlyPrimitiveStore {
                    source:
                        omega_assigned_target_operations::AssignedUnitWriteOnlyPrimitiveStoreSource::Home(home),
                    ..
                } => Some(home),
                _ => None,
            })
            .expect("assigned store reads the scalar result home");
        assert_eq!(assigned_store_home, assigned_result_home);

        let emitted = omega_machine_emission::emit_machine_code(&assigned)
            .expect("direct scalar-result store reaches machine emission");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("emitted direct scalar-result store owner is retained");
        let [producer] = caller.internal_unit_scalar_calls.as_slice() else {
            panic!("caller retains one scalar result producer")
        };
        let [emitted_store] = caller.unit_write_only_primitive_stores.as_slice() else {
            panic!("caller retains one direct write-only primitive store")
        };
        assert_eq!(
            emitted_store.source,
            omega_machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Home(producer.result.home)
        );
        assert_eq!(
            caller.bytes.get(
                emitted_store.code_offset..emitted_store.code_offset + emitted_store.byte_count
            ),
            Some(emitted_store.bytes.as_slice())
        );

        let mut changed_home = emitted.clone();
        let omega_machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Home(home) =
            &mut changed_home
                .functions
                .iter_mut()
                .find(|function| function.machine == emitted.entry)
                .unwrap()
                .unit_write_only_primitive_stores[0]
                .source
        else {
            unreachable!()
        };
        home.byte_offset = home.byte_offset.checked_add(8).unwrap();
        assert_eq!(
            omega_image_emission::build_object_artifact(&changed_home),
            Err(
                omega_image_emission::ObjectError::InvalidUnitWriteOnlyPrimitiveStoreEvidence(
                    emitted.entry
                )
            )
        );
        let mut changed_bytes = emitted.clone();
        changed_bytes
            .functions
            .iter_mut()
            .find(|function| function.machine == emitted.entry)
            .unwrap()
            .bytes[emitted_store.code_offset] ^= 1;
        assert_eq!(
            omega_image_emission::build_object_artifact(&changed_bytes),
            Err(
                omega_image_emission::ObjectError::InvalidUnitWriteOnlyPrimitiveStoreEvidence(
                    emitted.entry
                )
            )
        );

        let object = omega_image_emission::build_object_artifact(&emitted)
            .expect("object replay accepts the direct scalar-result store");
        let image = omega_image_emission::emit_executable_image(&object, 3)
            .expect("direct scalar-result store reaches an executable image");
        let installation = omega_image_emission::build_installation_record(
            &image,
            psi_core::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("installation retains the direct scalar-result store");
        let mut bytes = omega_image_emission::encode_installation_record(&installation)
            .expect("encode direct scalar-result store installation");
        let decoded = omega_image_emission::decode_installation_record(&bytes)
            .expect("decode direct scalar-result store installation");
        assert_eq!(decoded, installation);
        omega_image_emission::validate_installation_record(&decoded, &image)
            .expect("installation replays the direct scalar-result store");

        let encoded_store = bytes
            .windows(emitted_store.bytes.len())
            .rposition(|window| window == emitted_store.bytes)
            .expect("direct store bytes occur in the canonical installation record");
        bytes[encoded_store] ^= 1;
        assert!(matches!(
            omega_image_emission::decode_installation_record(&bytes),
            Err(
                omega_image_emission::InstallationError::InvalidUnitWriteOnlyPrimitiveStore(
                    machine
                )
            ) if machine == emitted.entry
        ));
    }
}

#[test]
fn scalar_result_home_directly_reaches_a_write_only_store_and_canonical_installation() {
    let checked = checked(
        r#"
            data Scalar {}
            machine Scalar::identity(value: i32) -> i32
            requires value == value
            ensures result == value
            {
                transition { _ -> value }
            }

            data Root {}
            machine Root::enter(destination: &mut i32) {
                let replacement: i32 = Scalar::identity(23);
                destination = replacement;
            }
        "#,
    );
    assert_scalar_result_home_directly_reaches_a_write_only_store_and_canonical_installation(
        &checked,
    );
}

#[test]
fn selected_result_home_directly_reaches_a_write_only_store_on_both_linux_targets() {
    let checked = checked_with_selected_integer_operator_store();
    assert_scalar_result_home_directly_reaches_a_write_only_store_and_canonical_installation(
        &checked,
    );
}
