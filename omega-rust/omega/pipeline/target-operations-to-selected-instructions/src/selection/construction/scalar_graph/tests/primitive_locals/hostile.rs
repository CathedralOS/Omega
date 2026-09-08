//! Hostile selected substitutions across local reentry and borrowed-call clobbers.
use super::*;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
};

#[test]
fn repeated_local_borrows_pass_the_original_pointer_in_exact_outgoing_stack_slots() {
    for (target, capacity) in [
        (target::NativeTarget::linux_x64(), 6),
        (target::NativeTarget::linux_arm64(), 8),
        (target::NativeTarget::windows_x64(), 4),
        (target::NativeTarget::macos_arm64(), 8),
    ] {
        for extra in [0, 2] {
            for unit_call in [false, true] {
                let mut source = local_fixture(target, unit_call);
                let scalar_count = capacity + extra;
                let LegalizedScalarInstructionKind::Call(call) =
                    &mut source.blocks[0].instructions[2].kind
                else {
                    panic!("borrowed call");
                };
                let mut reference = call.arguments[0].clone();
                call.call_plan = evaluate_call_plan(
                    CallingPolicy::native_for_target(target),
                    &CallSignature {
                        parameters: std::iter::repeat_n(ValueShape::integer(8, 8), scalar_count)
                            .chain([ValueShape::borrowed_reference(8, 8)])
                            .collect(),
                        result: call
                            .result_placement
                            .as_ref()
                            .map(|placement| placement.shape),
                    },
                )
                .unwrap();
                let LegalizedScalarArgument::Structural {
                    target: argument, ..
                } = &mut reference
                else {
                    panic!("local reference");
                };
                argument.destination = call.call_plan.parameters[scalar_count].clone();
                let offset =
                    crate::structural_reference_input::stack_pointer_offset(&argument.destination)
                        .expect("local reference follows all scalar registers");
                call.arguments = call.call_plan.parameters[..scalar_count]
                    .iter()
                    .map(|placement| LegalizedScalarArgument::Scalar {
                        source: ValueId::new(1).unwrap(),
                        placement: placement.clone(),
                    })
                    .chain([reference])
                    .collect();
                let second_call = source.blocks[0].instructions[2].kind.clone();
                append(&mut source, 5, second_call, (!unit_call).then_some(5));
                append(
                    &mut source,
                    6,
                    LegalizedScalarInstructionKind::PrimitiveScalarRead {
                        source: PlaceId::new(1).unwrap(),
                    },
                    Some(6),
                );
                let environment =
                    register_environment::baseline_target_register_environment(target).unwrap();
                let constraints = SelectedSelectionConstraints {
                    keys: environment.selected_keys(),
                    projected_structural_call: None,
                    fixed_inputs: Vec::new(),
                };
                let selected = build(
                    0,
                    &source,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
                .unwrap();
                let validate = |candidate: &SelectedFunction| {
                    crate::selection::validation::scalar_graph::validate(
                        0,
                        &source,
                        candidate,
                        target,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                    )
                };
                validate(&selected).unwrap();
                assert!(source.parameters.is_empty());
                assert_eq!(selected.local_storage_slots.len(), 1);
                let rows = &selected.blocks[0].instructions;
                let initialize = rows
                    .iter()
                    .find(|row| {
                        row.provenance.operations == [OperationId::new(2).unwrap()]
                            && matches!(row.kind, SelectedInstructionKind::Store { .. })
                    })
                    .unwrap();
                let original_pointer = initialize.operands[0].virtual_register;
                let initializer = initialize.operands[1].virtual_register;
                let snapshot = rows
                    .iter()
                    .find(|row| {
                        row.provenance.operations == [OperationId::new(4).unwrap()]
                            && matches!(row.kind, SelectedInstructionKind::Load64 { .. })
                    })
                    .unwrap()
                    .operands[1]
                    .virtual_register;
                for raw in [3, 5] {
                    let operation = OperationId::new(raw).unwrap();
                    let slot = selected_instructions::OutgoingArgumentSlotId {
                        operation,
                        argument_index: scalar_count as u32,
                    };
                    let placement = selected
                        .outgoing_arguments
                        .iter()
                        .find(|entry| entry.id == slot)
                        .unwrap();
                    assert_eq!(placement.abi_stack_byte_offset, offset);
                    assert_eq!(placement.byte_size, 8);
                    let stored = rows
                        .iter()
                        .position(|row| {
                            row.kind
                                == SelectedInstructionKind::Store64 {
                                    slot: FrameStorageSlotId::Outgoing(slot),
                                    byte_offset: 0,
                                }
                        })
                        .unwrap();
                    let forwarded = rows[stored].operands[0].virtual_register;
                    let copied = rows
                        .iter()
                        .position(|row| {
                            row.kind == SelectedInstructionKind::CopyI64
                                && row.operands[1].virtual_register == forwarded
                        })
                        .unwrap();
                    assert_eq!(rows[copied].operands[0].virtual_register, original_pointer);
                    assert_ne!(forwarded, original_pointer);
                    assert!(
                        selected
                            .memory_accesses
                            .iter()
                            .any(|access| access.instruction == rows[stored].id
                                && access.origin
                                    == SelectedMemoryAccessOrigin::Operation(operation)
                                && access.place == PlaceId::new(1).unwrap()
                                && access.byte_count == 8
                                && access.role == SelectedMemoryAccessRole::WriteOutgoing { slot })
                    );
                    for mutation in 0..5 {
                        let mut changed = selected.clone();
                        match mutation {
                            0 => {
                                changed.blocks[0].instructions[stored].operands[0]
                                    .virtual_register = initializer
                            }
                            1 => {
                                changed.blocks[0].instructions[copied].operands[0]
                                    .virtual_register = snapshot
                            }
                            2 => {
                                changed
                                    .outgoing_arguments
                                    .iter_mut()
                                    .find(|entry| entry.id == slot)
                                    .unwrap()
                                    .abi_stack_byte_offset += 8
                            }
                            3 => {
                                changed.blocks[0].instructions[stored].kind =
                                    SelectedInstructionKind::Store64 {
                                        slot: FrameStorageSlotId::Local(
                                            LocalStorageSlotId::Structural {
                                                operation: OperationId::new(2).unwrap(),
                                                place: PlaceId::new(1).unwrap(),
                                            },
                                        ),
                                        byte_offset: 0,
                                    }
                            }
                            _ => {
                                changed.blocks[0].instructions[stored].operands[0]
                                    .virtual_register = original_pointer
                            }
                        }
                        assert!(
                            validate(&changed).is_err(),
                            "outgoing mutation {mutation}, call {raw}"
                        );
                    }
                }
            }
        }
    }
}

pub(super) fn append(
    source: &mut LegalizedScalarFunction,
    raw: u64,
    kind: LegalizedScalarInstructionKind,
    result: Option<u64>,
) {
    let operation = OperationId::new(raw).unwrap();
    let scalar_type = source.blocks[0].instructions[0].result.unwrap().scalar_type;
    let ownership = if matches!(kind, LegalizedScalarInstructionKind::Call(_)) {
        vec![optimization_unit::OwnershipEvent::ClaimTransfer(Vec::new())]
    } else {
        Vec::new()
    };
    let block = &mut source.blocks[0];
    block.instructions.push(LegalizedScalarInstruction {
        operation,
        result: result.map(|value| LegalizedValueDefinition {
            value: ValueId::new(value).unwrap(),
            scalar_type,
            definition_site: ValueDefinitionSite::Node {
                block: block.id,
                node: block.instructions.len() as u32,
            },
        }),
        kind,
        fuel: vec![FuelSettlement {
            site: PsiProvenance::Operation(operation),
            units: 1,
        }],
        effect: EffectLink {
            input: raw - 1,
            output: raw,
        },
        ownership,
    });
    source.provenance.operations.push(operation);
}

fn clobber_fixture(target: target::NativeTarget) -> LegalizedScalarFunction {
    let mut source = local_fixture(target, false);
    let place = PlaceId::new(1).unwrap();
    let scalar = source.blocks[0].instructions[0].result.unwrap().scalar_type;
    let call = source.blocks[0].instructions[2].kind.clone();
    append(&mut source, 5, call, Some(5));
    append(
        &mut source,
        6,
        LegalizedScalarInstructionKind::PrimitiveScalarRead { source: place },
        Some(6),
    );
    append(
        &mut source,
        7,
        LegalizedScalarInstructionKind::PrimitiveLocalStore {
            destination: place,
            value: abstract_operations::AbstractResult {
                value: ValueId::new(3).unwrap(),
                scalar_type: scalar,
            },
        },
        None,
    );
    append(
        &mut source,
        8,
        LegalizedScalarInstructionKind::PrimitiveScalarRead { source: place },
        Some(8),
    );
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 4],
            result: None,
        },
    )
    .unwrap();
    let arguments = [4, 6, 8, 5]
        .into_iter()
        .zip(&call_plan.parameters)
        .map(|(raw, placement)| LegalizedScalarArgument::Scalar {
            source: ValueId::new(raw).unwrap(),
            placement: placement.clone(),
        })
        .collect();
    append(
        &mut source,
        9,
        LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
            source: LegalizedCallUnitSource::AuthoredCallUnit,
            callee: MachineId::new(11).unwrap(),
            call_plan,
            arguments,
            result_placement: None,
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        }),
        None,
    );
    source
}

#[test]
fn local_reads_and_scalar_results_survive_a_second_borrowed_call_without_substitution() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let source = clobber_fixture(target);
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        let selected = build(
            0,
            &source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let validate = |candidate: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                candidate,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&selected).unwrap();
        let demanded = crate::selection::value_transport::required_values(&source);
        assert_eq!(
            demanded,
            crate::selection::validation::value_transport::required_values(&source)
        );
        for raw in [1, 3, 4, 5, 6, 8] {
            assert!(
                demanded.contains(&ValueId::new(raw).unwrap()),
                "live scalar {raw}"
            );
        }
        let rows = &selected.blocks[0].instructions;
        let position = |raw, predicate: fn(&SelectedInstructionKind) -> bool| {
            rows.iter()
                .position(|row| {
                    row.provenance.operations == [OperationId::new(raw).unwrap()]
                        && predicate(&row.kind)
                })
                .unwrap()
        };
        let initialize = position(2, |kind| {
            matches!(kind, SelectedInstructionKind::Store { .. })
        });
        let first_call = position(3, |kind| {
            matches!(kind, SelectedInstructionKind::CallI64 { .. })
        });
        let first_read = position(4, |kind| {
            matches!(kind, SelectedInstructionKind::Load64 { .. })
        });
        let second_call = position(5, |kind| {
            matches!(kind, SelectedInstructionKind::CallI64 { .. })
        });
        let second_read = position(6, |kind| {
            matches!(kind, SelectedInstructionKind::Load64 { .. })
        });
        let replacement = position(7, |kind| {
            matches!(kind, SelectedInstructionKind::Store { .. })
        });
        let pointer = rows[initialize].operands[0].virtual_register;
        let initializer = rows[initialize].operands[1].virtual_register;
        let first_snapshot = rows[first_read].operands[1].virtual_register;
        let second_snapshot = rows[second_read].operands[1].virtual_register;
        let short_result = rows[first_call].operands.last().unwrap().virtual_register;
        let preserved_result = rows[first_call + 1].operands[1].virtual_register;
        assert!(first_call < first_read && first_read < second_call && second_call < second_read);
        assert_eq!(rows[first_read].operands[0].virtual_register, pointer);
        assert_eq!(rows[second_read].operands[0].virtual_register, pointer);
        assert_ne!(second_snapshot, first_snapshot);
        assert_ne!(second_snapshot, initializer);
        assert_ne!(preserved_result, short_result);
        assert_eq!(
            rows[replacement].operands[1].virtual_register,
            preserved_result
        );
        assert!(second_call < replacement);
        assert_eq!(selected.local_storage_slots.len(), 1);
        for mutation in 0..7 {
            let mut changed = selected.clone();
            match mutation {
                0 => {
                    let row = &mut changed.blocks[0].instructions[second_read];
                    row.kind = SelectedInstructionKind::CopyI64;
                    row.constraint = constraints.keys.copy_i64;
                    row.operands[0].virtual_register = initializer;
                    changed
                        .memory_accesses
                        .retain(|access| access.instruction != rows[second_read].id);
                }
                1 => {
                    changed.blocks[0].instructions[second_read].operands[0].virtual_register =
                        first_snapshot
                }
                2 => {
                    changed.blocks[0].instructions[replacement].operands[1].virtual_register =
                        short_result
                }
                3 => {
                    changed.blocks[0].instructions.remove(first_call + 1);
                }
                4 => {
                    changed.blocks[0].instructions.remove(initialize);
                    changed
                        .memory_accesses
                        .retain(|access| access.instruction != rows[initialize].id);
                }
                5 => {
                    changed.blocks[0].instructions[first_call].operands[0].virtual_register =
                        initializer
                }
                _ => {
                    let row = &mut changed.blocks[0].instructions[second_read];
                    row.kind = SelectedInstructionKind::CopyI64;
                    row.constraint = constraints.keys.copy_i64;
                    row.operands[0].virtual_register = first_snapshot;
                    changed
                        .memory_accesses
                        .retain(|access| access.instruction != rows[second_read].id);
                }
            }
            assert!(
                validate(&changed).is_err(),
                "clobber substitution {mutation}"
            );
        }
    }
}
