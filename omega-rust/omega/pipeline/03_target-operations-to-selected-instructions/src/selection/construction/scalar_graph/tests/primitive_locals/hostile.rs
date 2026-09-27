//! Hostile selected substitutions across local reentry and borrowed-call clobbers.
use super::super::{
    EffectLink, FuelSettlement, LegalizedScalarInstruction, LegalizedValueDefinition,
    PsiProvenance, ValueDefinitionSite,
};
use super::{
    CallSignature, CallingPolicy, LegalizedScalarArgument, LegalizedScalarFunction,
    LegalizedScalarInstructionKind, OperationId, PlaceId, SelectedFunction,
    SelectedInstructionKind, SelectedSelectionConstraints, ValueId, ValueShape, build,
    evaluate_call_plan,
};
use crate::selection::construction::scalar_graph::tests::primitive_locals::local_fixture;
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
                    crate::structural_inputs::structural_reference_input::stack_pointer_offset(
                        &argument.destination,
                    )
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
                        path: Vec::new(),
                        indices: Vec::new(),
                        source: PlaceId::new(1).unwrap(),
                    },
                    Some(6),
                );
                let environment =
                    register_environment::baseline_target_register_environment(target).unwrap();
                let constraints = SelectedSelectionConstraints {
                    keys: environment.selected_keys(),
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
                        role: selected_instructions::OutgoingArgumentSlotRole::Argument,
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
