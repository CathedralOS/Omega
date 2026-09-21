//! Register call signatures carry their authored arity, not a pair-shaped recipe.

use crate::tests::{
    AdmissionProfile, EdgeId, IntegerValue, NativeTarget, Operation, OperationId, OperationKind,
    OperationResult, Optimization, OptimizationSelections, OptimizedTargetLoweringRequest,
    SCALAR_CALL_UNIT_FIRST_RESULT, SCALAR_CALL_UNIT_LEFT, SCALAR_CALL_UNIT_RETURN_EDGE,
    SCALAR_CALL_UNIT_RIGHT, TerminalMachineResult, Terminator, ValueDeclaration, ValueId,
    canonical_artifact, compiler_baseline_request_v1, lower_optimized_to_target_operations,
    optimize_artifact_sections, scalar_call_unit_artifact_with,
    stage_function_fragment_frame_application, stage_optimized_fixed_frame_text_section,
    stage_optimized_function_fragment_emission, stage_optimized_instruction_selection,
    stage_optimized_relocation_free_object_container,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
    stage_validated_optimized_object_artifact, stage_validated_optimized_ordinary_callable_entry,
};
fn register_targets() -> [(NativeTarget, usize); 5] {
    [
        (NativeTarget::linux_x64(), 6),
        (NativeTarget::windows_x64(), 4),
        (NativeTarget::uefi_x64(), 4),
        (NativeTarget::linux_arm64(), 8),
        (NativeTarget::macos_arm64(), 8),
    ]
}

fn artifact(argument_count: usize) -> (Vec<u8>, Vec<u8>) {
    scalar_call_unit_artifact_with(|module| {
        let callee = &mut module.machines[1];
        let parameter = callee.parameters[0];
        callee.parameters = (0..argument_count)
            .map(|index| ValueDeclaration {
                qualifications: Default::default(),
                id: ValueId::new(24_000 + index as u64).unwrap(),
                scalar_type: parameter.scalar_type,
            })
            .collect();
        callee.blocks.truncate(1);
        callee.blocks[0].operations.clear();
        let returned = match callee.parameters.last() {
            Some(parameter) => parameter.id,
            None => {
                let value = ValueId::new(24_100).unwrap();
                callee.blocks[0].operations.push(Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: OperationId::new(24_101).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: value,
                        scalar_type: parameter.scalar_type,
                    }),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(19),
                    },
                });
                value
            }
        };
        callee.blocks[0].terminator = Terminator::Return {
            edge: EdgeId::new(24_102).unwrap(),
            value: returned,
            cleanup_actions: Vec::new(),
        };
        let operations = &mut module.machines[0].blocks[0].operations;
        operations.truncate(3);
        let OperationKind::Call { arguments, .. } = &mut operations[2].kind else {
            unreachable!()
        };
        *arguments = (0..argument_count)
            .map(|index| {
                ValueId::new(if index % 2 == 0 {
                    SCALAR_CALL_UNIT_LEFT
                } else {
                    SCALAR_CALL_UNIT_RIGHT
                })
                .unwrap()
            })
            .collect();
    })
}

#[test]
fn register_argument_rosters_use_shared_selection() {
    for (target, maximum) in register_targets() {
        for argument_count in 0..=maximum {
            let (semantic, proof) = artifact(argument_count);
            let selections = OptimizationSelections::new([]).unwrap();
            let optimized = optimize_artifact_sections(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                compiler_baseline_request_v1(&selections),
            )
            .unwrap();
            let lowered = lower_optimized_to_target_operations(
                optimized,
                OptimizedTargetLoweringRequest::new(target),
            )
            .unwrap();
            stage_optimized_instruction_selection(lowered).unwrap();
        }
    }
}

#[test]
fn register_call_arity_reaches_image_and_installation_with_empty_and_selected_phases() {
    for (target, maximum) in register_targets() {
        for argument_count in [0, 1, 3, maximum] {
            for choices in [Vec::new(), vec![Optimization::CopyPropagation]] {
                let (semantic, proof) = artifact(argument_count);
                let selections = OptimizationSelections::new(choices).unwrap();
                let optimized = optimize_artifact_sections(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    compiler_baseline_request_v1(&selections),
                )
                .unwrap();
                let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
                    optimized,
                    target,
                    &[],
                )
                .unwrap_or_else(|error| panic!("{target:?}, arity {argument_count}: {error:?}"));
                let emitted = stage_optimized_function_fragment_emission(
                    physical.into_function_fragment_emission_source(),
                )
                .unwrap();
                let framed = stage_function_fragment_frame_application(emitted).unwrap();
                let text = stage_optimized_fixed_frame_text_section(framed).unwrap();
                let source = stage_optimized_relocation_free_object_container(text).unwrap();
                let source = std::sync::Arc::new(source);
                let object =
                    image_emission::build_function_fragment_object_artifact(source.clone())
                        .unwrap();
                image_emission::validate_function_fragment_object_artifact(&source, &object)
                    .unwrap();
                assert_eq!(object.entry_function().unit_call_stacks.len(), 1);
                let demand = image_emission::derive_stack_demand(&object, object.entry()).unwrap();
                let image = image_emission::emit_executable_image(&object, 3).unwrap();
                image_emission::validate_executable_image(&object, &image).unwrap();
                let record = image_emission::build_installation_record(
                    &image,
                    semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
                )
                .unwrap();
                let encoded = image_emission::encode_installation_record(&record).unwrap();
                let decoded = image_emission::decode_installation_record(&encoded).unwrap();
                image_emission::validate_installation_record(&decoded, &image).unwrap();
                assert_eq!(
                    image_emission::derive_installation_stack_demand(
                        &decoded,
                        &image,
                        object.entry()
                    )
                    .unwrap(),
                    demand,
                );
            }
        }
    }
}

/// Caller and callee machines where one call argument spills past the
/// register capacity onto the outgoing stack area. The caller keeps a bare
/// scalar signature so its frame accesses can replay through ordinary
/// callable publication.
fn scalar_stack_argument_artifact(argument_count: usize) -> (Vec<u8>, Vec<u8>) {
    scalar_call_unit_artifact_with(|module| {
        let callee = &mut module.machines[1];
        let parameter = callee.parameters[0];
        callee.parameters = (0..argument_count)
            .map(|index| ValueDeclaration {
                qualifications: Default::default(),
                id: ValueId::new(24_000 + index as u64).unwrap(),
                scalar_type: parameter.scalar_type,
            })
            .collect();
        callee.blocks.truncate(1);
        callee.blocks[0].operations.clear();
        callee.blocks[0].terminator = Terminator::Return {
            edge: EdgeId::new(24_102).unwrap(),
            value: callee.parameters.last().unwrap().id,
            cleanup_actions: Vec::new(),
        };
        let caller = &mut module.machines[0];
        caller.attachment = None;
        caller.result = TerminalMachineResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(24_200).unwrap(),
            scalar_type: parameter.scalar_type,
        });
        let operations = &mut caller.blocks[0].operations;
        operations.truncate(3);
        let OperationKind::Call { arguments, .. } = &mut operations[2].kind else {
            unreachable!()
        };
        *arguments = (0..argument_count)
            .map(|index| {
                ValueId::new(if index % 2 == 0 {
                    SCALAR_CALL_UNIT_LEFT
                } else {
                    SCALAR_CALL_UNIT_RIGHT
                })
                .unwrap()
            })
            .collect();
        caller.blocks[0].terminator = Terminator::Return {
            edge: EdgeId::new(SCALAR_CALL_UNIT_RETURN_EDGE).unwrap(),
            value: ValueId::new(SCALAR_CALL_UNIT_FIRST_RESULT).unwrap(),
            cleanup_actions: Vec::new(),
        };
    })
}

/// Recomputes the physical displacement an encoded access must carry under
/// the validated frame row, replaying the same equations the frame-address
/// resolver applies so the test independently checks every symbolic access.
fn expected_frame_displacement(
    function: &::physical_instructions::PostAllocationMachineFunction,
    frame: &machine_code::FunctionTargetFrameLayout,
    symbolic: &::physical_instructions::PhysicalAddressOperation,
) -> u64 {
    use ::physical_instructions::PhysicalAddressOperation as Address;
    use ::selected_instructions::FrameStorageSlotId;
    let frame_slot_start = |slot: FrameStorageSlotId| match slot {
        FrameStorageSlotId::Incoming {
            abi_stack_byte_offset,
            ..
        } => {
            let return_bytes = match frame.return_address {
                machine_code::ReturnAddressFrameCustody::CallerActivationStack {
                    size_bytes,
                    ..
                } => u64::from(size_bytes),
                _ => 0,
            };
            frame.frame_size_bytes + return_bytes + u64::from(abi_stack_byte_offset)
        }
        FrameStorageSlotId::Outgoing(id) => u64::from(
            function
                .outgoing_arguments
                .iter()
                .find(|slot| slot.id == id)
                .unwrap_or_else(|| panic!("unresolved outgoing slot {id:?}"))
                .abi_stack_byte_offset,
        ),
        FrameStorageSlotId::Local(id) => {
            frame
                .local_storage_slots
                .iter()
                .find(|slot| slot.id == id)
                .unwrap_or_else(|| panic!("unresolved local slot {id:?}"))
                .frame_offset_bytes
        }
    };
    match symbolic {
        Address::FrameAddress { slot, byte_offset } | Address::Store64 { slot, byte_offset } => {
            frame_slot_start(*slot) + u64::from(*byte_offset)
        }
        Address::Store { byte_offset, .. }
        | Address::AddressOffset { byte_offset, .. }
        | Address::Load8 { byte_offset, .. }
        | Address::Load16 { byte_offset, .. }
        | Address::Load32 { byte_offset, .. }
        | Address::Load64 { byte_offset, .. }
        | Address::LoadPacked { byte_offset, .. }
        | Address::StorePacked { byte_offset, .. } => u64::from(*byte_offset),
        Address::Load8Indexed { .. } => 0,
        Address::HostedReadByte { .. } | Address::HostedWriteByteI32 { .. } => {
            panic!("hosted address kinds do not occur in this fixture")
        }
        Address::SaveFloatingControl { .. } | Address::RestoreFloatingControl { .. } => {
            panic!("floating-control saves do not occur in this fixture")
        }
    }
}

/// Stack-passed scalar call arguments occupy the caller's outgoing ABI area
/// and the callee's incoming activation area. Every symbolic frame access is
/// independently replayed against the validated frame geometry, and the
/// whole program still reaches ordinary callable publication on each target.
#[test]
fn stack_argument_calls_replay_frame_accesses_through_callable_publication() {
    use ::physical_instructions::PhysicalAddressOperation as Address;
    use ::selected_instructions::FrameStorageSlotId;
    for (target, register_capacity) in register_targets() {
        let argument_count = register_capacity + 1;
        let (semantic, proof) = scalar_stack_argument_artifact(argument_count);
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&OptimizationSelections::new([]).unwrap()),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap_or_else(|error| panic!("{target:?} arity {argument_count} physical: {error:?}"));
        let realization = physical.fixed_frame_for_test();
        let layout = realization.frame().plan();
        let machine_plan = realization.machine().machine().plan();
        let module = terminal_codec::decode_module(&semantic).unwrap();
        let mut outgoing_accesses = 0usize;
        let mut incoming_accesses = 0usize;
        // Encoding rows follow the exact function/block/instruction roster the
        // machine plan presents; instruction ids repeat across machines.
        let mut rows = realization.encoding().rows().iter();
        for function in &machine_plan.functions {
            let frame = layout
                .functions
                .iter()
                .find(|row| row.machine == function.machine)
                .unwrap_or_else(|| {
                    panic!("{target:?} missing frame row for {:?}", function.machine)
                });
            for block in &function.blocks {
                for instruction in &block.instructions {
                    let row = rows.next().unwrap_or_else(|| {
                        panic!(
                            "{target:?} missing encoding row for {:?}",
                            instruction.instruction
                        )
                    });
                    assert_eq!(row.instruction, instruction.instruction, "{target:?}");
                    let Some(symbolic) = instruction.address else {
                        continue;
                    };
                    let resolved = row.address.unwrap_or_else(|| {
                        panic!(
                            "{target:?} unresolved address for {:?} symbolic {symbolic:?} row {row:?}",
                            instruction.instruction
                        )
                    });
                    assert_eq!(resolved.symbolic, symbolic, "{target:?}");
                    assert_eq!(
                        resolved.displacement,
                        i64::try_from(expected_frame_displacement(function, frame, &symbolic))
                            .expect("expected frame displacement fits a signed displacement"),
                        "{target:?} {symbolic:?}"
                    );
                    match symbolic {
                        Address::FrameAddress {
                            slot: FrameStorageSlotId::Outgoing(_),
                            ..
                        } => outgoing_accesses += 1,
                        Address::FrameAddress {
                            slot: FrameStorageSlotId::Incoming { .. },
                            ..
                        } => incoming_accesses += 1,
                        _ => {}
                    }
                }
            }
        }
        assert!(rows.next().is_none(), "{target:?} trailing encoding rows");
        let stack_arguments = argument_count - register_capacity;
        let entry_function = machine_plan
            .functions
            .iter()
            .find(|function| function.machine == module.entry)
            .unwrap();
        let entry_frame = layout
            .functions
            .iter()
            .find(|row| row.machine == module.entry)
            .unwrap();
        assert!(entry_frame.contains_call, "{target:?}");
        assert_eq!(
            entry_function.outgoing_arguments.len(),
            stack_arguments,
            "{target:?}"
        );
        assert_eq!(outgoing_accesses, stack_arguments, "{target:?}");
        assert_eq!(incoming_accesses, stack_arguments, "{target:?}");
        for slot in &entry_function.outgoing_arguments {
            assert!(
                u64::from(slot.abi_stack_byte_offset) + 8
                    <= entry_frame.outgoing_abi_area.byte_size,
                "{target:?} outgoing slot {slot:?} escapes the outgoing ABI area"
            );
        }
        assert!(
            entry_frame.frame_size_bytes >= entry_frame.outgoing_abi_area.byte_size,
            "{target:?} outgoing ABI area escapes the frame"
        );
        let emitted = stage_optimized_function_fragment_emission(
            physical.into_function_fragment_emission_source(),
        )
        .unwrap_or_else(|error| panic!("{target:?} arity {argument_count} emission: {error:?}"));
        let framed = stage_function_fragment_frame_application(emitted)
            .unwrap_or_else(|error| panic!("{target:?} arity {argument_count} frame: {error:?}"));
        let text = stage_optimized_fixed_frame_text_section(framed)
            .unwrap_or_else(|error| panic!("{target:?} arity {argument_count} text: {error:?}"));
        let object = stage_optimized_relocation_free_object_container(text)
            .unwrap_or_else(|error| panic!("{target:?} arity {argument_count} object: {error:?}"));
        let artifact = stage_validated_optimized_object_artifact(
            canonical_artifact(&semantic, &proof),
            object,
        )
        .unwrap_or_else(|error| panic!("{target:?} arity {argument_count} artifact: {error:?}"));
        let callable =
            stage_validated_optimized_ordinary_callable_entry(artifact).unwrap_or_else(|error| {
                panic!("{target:?} arity {argument_count} callable: {error:?}")
            });
        assert!(callable.entry().parameters.is_empty(), "{target:?}");
        assert_eq!(callable.entry().returns.len(), 1, "{target:?}");
    }
}
