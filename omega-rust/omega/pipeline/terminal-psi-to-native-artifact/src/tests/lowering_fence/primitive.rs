//! Focused write-only lowering and installation controls.

use super::*;

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
    let lowered = checked_trees_to_terminal_psi::lower_machine(&checked, "Root::enter")
        .expect("write-only store reaches verified Terminal production");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode write-only store semantics");
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode write-only store proof bundle");
    let abstract_plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("verified write-only store reaches target-neutral Omega");
    let x64_target = abstract_operations_to_target_operations::lower_to_target_operations(
        &abstract_plan,
        target::NativeTarget::linux_x64(),
    )
    .expect("verified whole-root store reaches target custody");
    let mut corrupted = x64_target.clone();
    let operation = corrupted
        .functions
        .iter_mut()
        .find_map(|function| match &mut function.operation {
            target_operations::TargetOperation::UnitBody(body) => {
                body.operations.iter_mut().find(|operation| {
                    matches!(
                        operation,
                        target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore { .. }
                    )
                })
            }
            _ => None,
        })
        .expect("target plan retains the write-only store");
    let target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore {
        destination_type, ..
    } = operation
    else {
        unreachable!()
    };
    destination_type.identity.push_str("::forged");
    assert!(matches!(
        target_operations_to_assigned_target_operations::assign_registers(&corrupted),
        Err(target_operations_to_assigned_target_operations::AssignmentError::WriteOnlyPrimitiveStoreCustodyMismatch { .. })
    ));

    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            target,
        )
        .expect("verified whole-root store reaches target custody");
        let assigned = target_operations_to_assigned_target_operations::assign_registers(&target)
            .expect("exact target store reaches independently replayed physical assignment");
        let emitted = machine_emission::emit_machine_code(&assigned)
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
            terminal_psi::StructuralAccess::WriteOnlyBorrow
        ));
        assert!(matches!(
            store.destination_type.shape,
            terminal_psi::StructuralTypeShape::PrimitiveScalar(
                semantic_vocabulary::ScalarType::Integer(_)
            )
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
        let rejects = |candidate: &machine_code::MachineCodePlan| {
            assert_eq!(
                image_emission::build_object_artifact(candidate),
                Err(
                    image_emission::ObjectError::InvalidUnitWriteOnlyPrimitiveStoreEvidence(
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
        let object = image_emission::build_object_artifact(&emitted)
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
        let image = image_emission::emit_executable_image(&object, 3)
            .expect("replayed store reaches an executable image");
        let installation = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
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
        let installation_bytes = image_emission::encode_installation_record(&installation)
            .expect("encode the installed store custody");
        let decoded = image_emission::decode_installation_record(&installation_bytes)
            .expect("decode the installed store custody");
        assert_eq!(decoded, installation);
        image_emission::validate_installation_record(&decoded, &image)
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
            image_emission::decode_installation_record(&corrupted_installation),
            Err(
                image_emission::InstallationError::InvalidUnitWriteOnlyPrimitiveStore(
                    machine
                )
            ) if machine == function.machine
        ));
    }
}

#[test]
fn finite_literal_write_only_subloan_reaches_both_linux_artifacts() {
    let checked = checked(
        r#"
            data Outer [copy] {
                prefix: u8;
                values: [[[[[[u16; 7]; 6]; 5]; 4]; 3]; 2];
            }

            data Sink {}
            machine Sink::fill(destination: &write u16, replacement: u16) {
                destination = replacement;
            }

            data Root {}
            machine Root::forward(outer: &write Outer, replacement: u16) {
                Sink::fill(&write outer.values[1][2][3][4][5][6], replacement);
            }
        "#,
    );
    let lowered = checked_trees_to_terminal_psi::lower_machine(&checked, "Root::forward")
        .expect("finite literal write-only subloan reaches verified Terminal");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode projected write-only semantics");
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode projected write-only proof bundle");
    let abstract_plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("verified projected write-only call reaches target-neutral Omega");

    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            target,
        )
        .expect("finite literal write-only subloan reaches target custody");
        let mut corrupted_target = target.clone();
        let target_argument = corrupted_target
            .functions
            .iter_mut()
            .find(|function| function.machine == corrupted_target.entry)
            .and_then(|function| match &mut function.operation {
                target_operations::TargetOperation::UnitBody(body) => body
                    .operations
                    .iter_mut()
                    .find_map(|operation| match operation {
                        target_operations::TargetUnitOperation::Call { arguments, .. } => {
                            arguments.first_mut()
                        }
                        _ => None,
                    }),
                _ => None,
            })
            .expect("target call retains its projected argument");
        target_argument.source_byte_offset += 2;
        assert!(matches!(
            target_operations_to_assigned_target_operations::assign_registers(
                &corrupted_target
            ),
            Err(
                target_operations_to_assigned_target_operations::AssignmentError::UnitCallCustodyMismatch { .. }
            )
        ));
        let assigned = target_operations_to_assigned_target_operations::assign_registers(&target)
            .expect("projected write-only pointer adjustment assigns");
        let emitted = machine_emission::emit_machine_code(&assigned)
            .expect("projected write-only pointer adjustment emits");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("entry caller");
        let [call] = caller.internal_unit_calls.as_slice() else {
            panic!("one projected write-only call must survive machine emission")
        };
        assert_eq!(call.scalar_arguments.len(), 1);
        let [argument] = call.arguments.as_slice() else {
            panic!("projected call must retain one write-only argument")
        };
        assert_eq!(
            argument.access,
            terminal_psi::StructuralAccess::WriteOnlyBorrow
        );
        assert_eq!(argument.path.len(), 7);
        assert_eq!(argument.source_byte_offset, 10_080);
        assert_eq!(argument.fixed_array_length, None);
        assert_eq!(argument.element_stride, None);
        assert_eq!(
            argument.shape.class,
            calling_conventions::ValueClass::BorrowedReference
        );
        assert!(!argument.bytes.is_empty());
        let callee = emitted
            .functions
            .iter()
            .find(|function| function.machine == call.target)
            .expect("write-only callee");
        assert_eq!(callee.unit_write_only_primitive_stores.len(), 1);

        let rejects = |candidate: &machine_code::MachineCodePlan| {
            assert!(matches!(
                image_emission::build_object_artifact(candidate),
                Err(image_emission::ObjectError::InvalidInternalUnitCallEvidence(
                    machine
                )) if machine == caller.machine
            ));
        };
        let mut changed_offset = emitted.clone();
        changed_offset
            .functions
            .iter_mut()
            .find(|function| function.machine == caller.machine)
            .unwrap()
            .internal_unit_calls[0]
            .arguments[0]
            .source_byte_offset += 2;
        rejects(&changed_offset);
        let mut changed_path = emitted.clone();
        let path = &mut changed_path
            .functions
            .iter_mut()
            .find(|function| function.machine == caller.machine)
            .unwrap()
            .internal_unit_calls[0]
            .arguments[0]
            .path;
        *path.last_mut().unwrap() = terminal_psi::StructuralPathSegment::FixedIndex(0);
        rejects(&changed_path);
        let mut changed_scalar = emitted.clone();
        let scalar_source = &mut changed_scalar
            .functions
            .iter_mut()
            .find(|function| function.machine == caller.machine)
            .unwrap()
            .internal_unit_calls[0]
            .scalar_arguments[0]
            .source;
        let machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
            source_value, ..
        } = scalar_source
        else {
            panic!("projected write-only call retains its scalar parameter")
        };
        *source_value = semantic_vocabulary::ValueId::new(9_999_999).unwrap();
        rejects(&changed_scalar);

        let object = image_emission::build_object_artifact(&emitted)
            .expect("object replay accepts the projected write-only call");
        let image = image_emission::emit_executable_image(&object, 3)
            .expect("projected write-only call reaches an executable image");
        let installation = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .expect("installation retains the projected write-only call");
        image_emission::validate_installation_record(&installation, &image)
            .expect("installation independently replays the projected write-only call");
    }
}
