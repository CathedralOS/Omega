//! Source-to-machine custody for the bounded projected store/call lane.

use crate::tests::fixtures::checked_source::{checked, checked_with_sole_selected_provider};

const SOURCE: &str = r#"
    trait Measure {
        machine measure(&self) -> i32;
    }

    data Item [copy] {
        value: i32;
    }

    Primary: Item satisfies Measure {
        machine measure(&self) -> i32 {
            transition { _ -> self.value }
        }
    }

    data Main [copy] {
        item: Item;
    }

    machine Main::run(&mut self) {
        self.item.value = 17;
        let erased: &dyn Measure = &self.item as &dyn Item::Primary;
        let result: i32 = erased.measure();
    }
"#;

const WRITE_ONLY_SOURCE: &str = r#"
    data Pair { prefix: u8; target: u16; }
    data Inner { prefix: u8; value: u16; }
    data Outer { prefix: u8; inner: Inner; }
    data Flags { prefix: u8; target: bool; }
    data Cell [copy] { prefix: u8; value: u16; }
    data Matrix { prefix: u8; cells: [Cell; 3]; }
    data Sink {}

    machine Sink::direct(pair: &write Pair) {
        pair.target = 17;
    }

    machine Sink::nested(outer: &write Outer) {
        outer.inner.value = 19;
    }

    machine Sink::parameter(pair: &write Pair, replacement: u16) {
        pair.target = replacement;
    }

    machine Sink::boolean_literal(flags: &write Flags) {
        flags.target = true;
    }

    machine Sink::boolean_parameter(flags: &write Flags, replacement: bool) {
        flags.target = replacement;
    }

    machine Sink::indexed(matrix: &write Matrix) {
        matrix.cells[2].value = 29;
    }

    machine Sink::stack_parameter(
        pair: &write Pair,
        a: u16,
        b: u16,
        c: u16,
        d: u16,
        e: u16,
        f: u16,
        g: u16,
        h: u16,
        replacement: u16
    ) {
        pair.target = replacement;
    }
"#;

const RESULT_SOURCED_STORE: &str = r#"
    data Scalar {}
    machine Scalar::identity(value: i32) -> i32
    requires value == value
    ensures result == value
    {
        transition { _ -> value }
    }

    data Pair { prefix: u8; target: i32; }
    data Root {}
    machine Root::enter(destination: &write Pair) {
        let replacement: i32 = Scalar::identity(23);
        destination.target = replacement;
    }
"#;

const SELECTED_RESULT_SOURCED_STORE: &str = r#"
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

    data Pair { prefix: u8; target: i32; }
    data Root {}
    machine Root::enter(destination: &write Pair) {
        let replacement: i32 = CheckedMath::offset_zero(23);
        destination.target = replacement;
    }
"#;

#[test]
fn direct_dynamic_projected_store_and_call_reach_machine_custody() {
    let checked = checked(SOURCE);
    let terminal = terminal_production::produce_terminal_artifact(&checked, "Main::run")
        .expect("direct dynamic source reaches canonical Terminal");
    let abstract_plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
        terminal.semantic_bytes(),
        terminal.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("verified direct dynamic source reaches target-neutral Omega");

    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let target_plan = abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            target,
        )
        .expect("projected store and call reach target operations");
        let assigned =
            target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                .expect("projected store and call retain physical assignment");
        let emitted = machine_emission::emit_machine_code(&assigned)
            .expect("projected store and call reach machine emission");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("entry caller");
        let [store] = caller.unit_structural_scalar_field_stores.as_slice() else {
            panic!("one projected field store must survive machine emission")
        };
        assert_eq!(store.psi_operation.get(), 2);
        assert_eq!(store.field_byte_offset, 0);
        let parameter_home = caller
            .unit_parameter_homes
            .iter()
            .find(|home| home.place == store.destination.place)
            .expect("store destination has one exact staged parameter home");
        assert_eq!(store.parameter_home_byte_offset, parameter_home.byte_offset);
        assert_eq!(store.parameter_home_indirect, parameter_home.indirect);
        assert!(!store.bytes.is_empty());
        assert_eq!(
            &caller.bytes[store.code_offset..store.code_offset + store.byte_count],
            store.bytes
        );
        let [call] = caller.internal_unit_calls.as_slice() else {
            panic!("one projected structural scalar call must survive machine emission")
        };
        assert_eq!(
            call.result,
            Some(semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
                    .unwrap()
            ))
        );
        assert_eq!(call.arguments.len(), 1);
        assert_eq!(call.arguments[0].path.len(), 1);
        assert_eq!(call.arguments[0].source_byte_offset, 0);
    }
}

#[test]
fn parameter_sourced_write_only_field_store_reaches_canonical_installation() {
    assert_parameter_sourced_field_store("Sink::parameter", 2, 0, false);
    assert_parameter_sourced_field_store("Sink::boolean_parameter", 1, 0, false);
    assert_parameter_sourced_field_store("Sink::stack_parameter", 2, 8, true);
}

#[test]
fn scalar_result_home_reaches_a_projected_store_and_canonical_installation() {
    let checked = checked(RESULT_SOURCED_STORE);
    assert_scalar_result_home_reaches_a_projected_store_and_canonical_installation(&checked);
}

#[test]
fn selected_result_home_reaches_a_projected_store_on_both_linux_targets() {
    let checked = checked_with_sole_selected_provider(SELECTED_RESULT_SOURCED_STORE);
    assert_scalar_result_home_reaches_a_projected_store_and_canonical_installation(&checked);
}

fn assert_scalar_result_home_reaches_a_projected_store_and_canonical_installation(
    checked: &checked_trees::CheckedTrees,
) {
    let terminal = terminal_production::produce_terminal_artifact(checked, "Root::enter")
        .expect("projected scalar-result store reaches canonical Terminal");
    let abstract_plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
        terminal.semantic_bytes(),
        terminal.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("projected scalar-result store reaches target-neutral Omega");

    for native_target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            native_target,
        )
        .expect("projected scalar-result store reaches target custody");
        let target_body = target
            .functions
            .iter()
            .find(|function| function.machine == target.entry)
            .and_then(|function| match &function.operation {
                target_operations::TargetOperation::UnitBody(body) => Some(body),
                _ => None,
            })
            .expect("target caller remains a Unit body");
        let target_result_home = target_body
            .operations
            .iter()
            .find_map(|operation| match operation {
                target_operations::TargetUnitOperation::ScalarCall { result_home, .. } => {
                    Some(result_home)
                }
                _ => None,
            })
            .expect("target caller retains the scalar producer");
        let (target_store_home, target_field_offset) = target_body
            .operations
            .iter()
            .find_map(|operation| match operation {
                target_operations::TargetUnitOperation::StructuralScalarFieldStore {
                    source: target_operations::TargetUnitScalarArgumentSource::Home(home),
                    field_byte_offset,
                    ..
                } => Some((home, field_byte_offset)),
                _ => None,
            })
            .expect("target projected store reads the scalar result home");
        assert_eq!(target_store_home, target_result_home);
        assert_eq!(*target_field_offset, 4);

        let mut changed_target_home = target.clone();
        let changed_source = changed_target_home
            .functions
            .iter_mut()
            .find(|function| function.machine == changed_target_home.entry)
            .and_then(|function| match &mut function.operation {
                target_operations::TargetOperation::UnitBody(body) => body
                    .operations
                    .iter_mut()
                    .find_map(|operation| match operation {
                        target_operations::TargetUnitOperation::StructuralScalarFieldStore {
                            source,
                            ..
                        } => Some(source),
                        _ => None,
                    }),
                _ => None,
            })
            .expect("changed target retains the projected store");
        let target_operations::TargetUnitScalarArgumentSource::Home(home) = changed_source else {
            unreachable!()
        };
        home.source_value =
            semantic_vocabulary::ValueId::new(home.source_value.get() + 100).unwrap();
        assert!(matches!(
            target_operations_to_assigned_target_operations::assign_registers(
                &changed_target_home
            ),
            Err(
                target_operations_to_assigned_target_operations::AssignmentError::StructuralScalarFieldStoreCustodyMismatch { .. }
            )
        ));

        let assigned = target_operations_to_assigned_target_operations::assign_registers(&target)
            .expect("projected scalar-result store reaches physical assignment");
        let assigned_body = assigned
            .functions
            .iter()
            .find(|function| function.machine == assigned.entry)
            .and_then(|function| match &function.operation {
                assigned_target_operations::AssignedOperation::UnitBody(body) => Some(body),
                _ => None,
            })
            .expect("assigned caller remains a Unit body");
        let assigned_result_home = assigned_body
            .operations
            .iter()
            .find_map(|operation| match operation {
                assigned_target_operations::AssignedUnitOperation::ScalarCall {
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
                assigned_target_operations::AssignedUnitOperation::StructuralScalarFieldStore {
                    source: assigned_target_operations::AssignedUnitScalarArgumentSource::Home(home),
                    ..
                } => Some(home),
                _ => None,
            })
            .expect("assigned projected store reads the scalar result home");
        assert_eq!(assigned_store_home, assigned_result_home);

        let emitted = machine_emission::emit_machine_code(&assigned)
            .expect("projected scalar-result store reaches machine emission");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("emitted caller");
        let [producer] = caller.internal_unit_scalar_calls.as_slice() else {
            panic!("caller retains one scalar result producer")
        };
        let [store] = caller.unit_structural_scalar_field_stores.as_slice() else {
            panic!("caller retains one projected scalar-result store")
        };
        assert_eq!(store.field_byte_offset, 4);
        assert_eq!(
            store.source,
            machine_code::InternalUnitScalarArgumentSourceRecord::Home(producer.result.home)
        );
        assert_eq!(
            caller
                .bytes
                .get(store.code_offset..store.code_offset + store.byte_count),
            Some(store.bytes.as_slice())
        );

        let mut changed_home = emitted.clone();
        let machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) = &mut changed_home
            .functions
            .iter_mut()
            .find(|function| function.machine == emitted.entry)
            .unwrap()
            .unit_structural_scalar_field_stores[0]
            .source
        else {
            unreachable!()
        };
        home.byte_offset = home.byte_offset.checked_add(8).unwrap();
        assert_eq!(
            image_emission::build_object_artifact(&changed_home),
            Err(
                image_emission::ObjectError::InvalidUnitStructuralScalarFieldStoreEvidence(
                    emitted.entry,
                ),
            )
        );

        let object = image_emission::build_object_artifact(&emitted)
            .expect("object replay accepts the projected scalar-result store");
        let image = image_emission::emit_executable_image(&object, 3)
            .expect("projected scalar-result store reaches an executable image");
        let installation = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("installation retains the projected scalar-result store");
        let encoded = image_emission::encode_installation_record(&installation)
            .expect("encode projected scalar-result store custody");
        let decoded = image_emission::decode_installation_record(&encoded)
            .expect("decode projected scalar-result store custody");
        assert_eq!(decoded, installation);
        image_emission::validate_installation_record(&decoded, &image)
            .expect("installation independently replays the projected scalar-result store");
        let mut changed_encoded = encoded;
        let store_bytes = store.bytes.clone();
        let encoded_store = changed_encoded
            .windows(store_bytes.len())
            .rposition(|window| window == store_bytes)
            .expect("projected store bytes occur in the canonical installation record");
        changed_encoded[encoded_store] ^= 1;
        assert!(matches!(
            image_emission::decode_installation_record(&changed_encoded),
            Err(
                image_emission::InstallationError::InvalidUnitStructuralScalarFieldStore(
                    machine
                )
            ) if machine == emitted.entry
        ));
    }
}

fn assert_parameter_sourced_field_store(
    machine: &str,
    expected_field_byte_offset: u32,
    expected_source_index: u32,
    expect_stack_source: bool,
) {
    let checked = checked(WRITE_ONLY_SOURCE);
    let terminal = terminal_production::produce_terminal_artifact(&checked, machine)
        .expect("parameter-sourced field store reaches canonical Terminal");
    let abstract_plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
        terminal.semantic_bytes(),
        terminal.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("verified parameter-sourced field store reaches target-neutral Omega");

    for native_target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            native_target,
        )
        .expect("parameter-sourced field store reaches target custody");
        let body = target
            .functions
            .iter()
            .find_map(|function| match &function.operation {
                target_operations::TargetOperation::UnitBody(body) => Some(body),
                _ => None,
            })
            .expect("one target Unit body");
        let scalar_parameter = body
            .scalar_parameters
            .get(usize::try_from(expected_source_index).unwrap())
            .expect("selected target scalar parameter");
        let source = body
            .operations
            .iter()
            .find_map(|operation| match operation {
                target_operations::TargetUnitOperation::StructuralScalarFieldStore {
                    source,
                    ..
                } => Some(source),
                _ => None,
            })
            .expect("one target projected store");
        assert!(matches!(
            source,
            target_operations::TargetUnitScalarArgumentSource::Parameter {
                parameter_index,
                source_value,
                scalar_type,
            } if *parameter_index == expected_source_index
                && *source_value == scalar_parameter.value
                && *scalar_type == scalar_parameter.scalar_type
        ));

        let assigned = target_operations_to_assigned_target_operations::assign_registers(&target)
            .expect("parameter-sourced field store reaches physical assignment");
        let emitted = machine_emission::emit_machine_code(&assigned)
            .expect("parameter-sourced field store reaches machine emission");
        let function = emitted
            .functions
            .iter()
            .find(|function| function.unit_structural_scalar_field_stores.len() == 1)
            .expect("one machine owns the parameter-sourced field store");
        let store = &function.unit_structural_scalar_field_stores[0];
        assert_eq!(store.field_byte_offset, expected_field_byte_offset);
        let machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index,
            source_value,
            scalar_type,
            location,
        } = store.source
        else {
            panic!("machine store retains one parameter source")
        };
        assert_eq!(parameter_index, expected_source_index);
        assert_eq!(source_value, scalar_parameter.value);
        assert_eq!(scalar_type, scalar_parameter.scalar_type);
        match (expect_stack_source, location) {
            (false, machine_code::UnitScalarParameterLocationRecord::Register(register)) => {
                assert_eq!(register.architecture(), native_target.architecture)
            }
            (true, machine_code::UnitScalarParameterLocationRecord::IncomingStack { .. }) => {}
            _ => panic!("parameter source has the expected ABI location family"),
        }

        let mut corrupted = emitted.clone();
        let corrupted_source = &mut corrupted
            .functions
            .iter_mut()
            .find(|candidate| candidate.machine == function.machine)
            .unwrap()
            .unit_structural_scalar_field_stores[0]
            .source;
        let machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index, ..
        } = corrupted_source
        else {
            unreachable!()
        };
        *parameter_index = u32::MAX;
        assert!(image_emission::build_object_artifact(&corrupted).is_err());

        let object = image_emission::build_object_artifact(&emitted)
            .expect("object replay accepts the parameter-sourced field store");
        let image = image_emission::emit_executable_image(&object, 3)
            .expect("parameter-sourced field store reaches an executable image");
        let installation = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("installation retains the parameter-sourced field store");
        let encoded = image_emission::encode_installation_record(&installation)
            .expect("encode parameter-sourced field-store custody");
        let decoded = image_emission::decode_installation_record(&encoded)
            .expect("decode parameter-sourced field-store custody");
        assert_eq!(decoded, installation);
        image_emission::validate_installation_record(&decoded, &image)
            .expect("installation independently replays the parameter-sourced field store");
    }
}

#[test]
fn record_and_literal_indexed_write_only_field_stores_reach_both_linux_targets() {
    let checked = checked(WRITE_ONLY_SOURCE);
    for (machine_name, path_len, field_byte_offset) in [
        ("Sink::direct", 0_usize, 2_u32),
        ("Sink::nested", 1, 4),
        ("Sink::indexed", 2, 12),
        ("Sink::boolean_literal", 0, 1),
    ] {
        let terminal = terminal_production::produce_terminal_artifact(&checked, machine_name)
            .expect("write-only field store reaches canonical Terminal");
        let abstract_plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
            terminal.semantic_bytes(),
            terminal.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("verified write-only field store reaches target-neutral Omega");

        for target in [
            target::NativeTarget::linux_x64(),
            target::NativeTarget::linux_arm64(),
        ] {
            let target_plan = abstract_operations_to_target_operations::lower_to_target_operations(
                &abstract_plan,
                target,
            )
            .expect("write-only field store reaches target custody");
            if machine_name == "Sink::indexed" {
                let mut changed = target_plan.clone();
                let entry = changed.entry;
                let path = changed
                    .functions
                    .iter_mut()
                    .find(|function| function.machine == entry)
                    .and_then(|function| match &mut function.operation {
                        target_operations::TargetOperation::UnitBody(body) => {
                            body.operations.iter_mut().find_map(|operation| {
                                match operation {
                                target_operations::TargetUnitOperation::StructuralScalarFieldStore {
                                    path,
                                    ..
                                } => Some(path),
                                _ => None,
                            }
                            })
                        }
                        _ => None,
                    })
                    .expect("literal-indexed target store path");
                path[1] = terminal_psi::StructuralPathSegment::FixedIndex(3);
                assert!(matches!(
                    target_operations_to_assigned_target_operations::assign_registers(
                        &changed
                    ),
                    Err(
                        target_operations_to_assigned_target_operations::AssignmentError::StructuralScalarFieldStoreCustodyMismatch { .. }
                    )
                ));
            }
            let assigned =
                target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                    .expect("write-only field store retains physical assignment");
            let emitted = machine_emission::emit_machine_code(&assigned)
                .expect("write-only field store reaches machine emission");
            let function = emitted
                .functions
                .iter()
                .find(|function| function.machine == emitted.entry)
                .expect("entry function");
            let [store] = function.unit_structural_scalar_field_stores.as_slice() else {
                panic!("one write-only field store must survive machine emission")
            };
            assert!(!store.destination.is_self);
            assert_eq!(
                store.destination.access,
                terminal_psi::StructuralAccess::WriteOnlyBorrow
            );
            assert_eq!(store.path.len(), path_len);
            assert_eq!(store.field_byte_offset, field_byte_offset);
            let home = function
                .unit_parameter_homes
                .iter()
                .find(|home| home.place == store.destination.place)
                .expect("store destination home");
            assert_eq!(store.destination_placement, home.source);
            assert_eq!(store.parameter_home_byte_offset, home.byte_offset);
            assert_eq!(store.parameter_home_indirect, home.indirect);
            assert_eq!(
                &function.bytes[store.code_offset..store.code_offset + store.byte_count],
                store.bytes
            );

            let object = image_emission::build_object_artifact(&emitted)
                .expect("object replay accepts the ordinary parameter store");
            let image = image_emission::emit_executable_image(&object, 3)
                .expect("write-only field store reaches an executable image");
            let installation = image_emission::build_installation_record(
                &image,
                semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
            )
            .expect("installation retains the write-only field store");
            image_emission::validate_installation_record(&installation, &image)
                .expect("installation independently replays the field store");

            let mut corrupted = emitted.clone();
            corrupted
                .functions
                .iter_mut()
                .find(|candidate| candidate.machine == function.machine)
                .expect("store-owning function")
                .unit_structural_scalar_field_stores[0]
                .bytes[0] ^= 1;
            assert_eq!(
                image_emission::build_object_artifact(&corrupted),
                Err(
                    image_emission::ObjectError::InvalidUnitStructuralScalarFieldStoreEvidence(
                        function.machine,
                    ),
                )
            );
        }
    }
}
