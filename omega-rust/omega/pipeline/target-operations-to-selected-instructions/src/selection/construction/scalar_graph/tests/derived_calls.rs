//! Raw source custody and selected SSA replay; these tests mint no source stamp.
use super::*;
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId};
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};

fn derived_call(target: target::NativeTarget, empty: bool) -> LegalizedScalarFunction {
    let mut source = subslices::view_fixture(target, empty);
    source.blocks[0].instructions.truncate(4);
    let mut call = borrowed_calls::borrowed_call(target).blocks[0]
        .instructions
        .remove(0);
    let operation = OperationId::new(4).unwrap();
    call.operation = operation;
    let definition = call.result.as_mut().unwrap();
    definition.value = ValueId::new(4).unwrap();
    definition.definition_site = ValueDefinitionSite::Node {
        block: source.entry_block,
        node: 3,
    };
    call.fuel = vec![FuelSettlement {
        site: PsiProvenance::Operation(operation),
        units: 1,
    }];
    let LegalizedScalarInstructionKind::Call(arguments) = &mut call.kind else {
        panic!("borrowed call")
    };
    let LegalizedScalarArgument::Structural { semantic, target } = &mut arguments.arguments[0]
    else {
        panic!("view")
    };
    semantic.place = PlaceId::new(2).unwrap();
    target.place = semantic.place;
    target.source = target_operations::TargetStructuralArgumentSource::EstablishedByteView {
        psi_operation: OperationId::new(3).unwrap(),
    };
    source.blocks[0].instructions[3] = call;
    source.structural.as_mut().unwrap().structural_places.push(
        terminal_psi::StructuralPlaceDeclaration {
            id: PlaceId::new(2).unwrap(),
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(3).unwrap(),
                structural_type: StructuralTypeId::new(1).unwrap(),
            },
        },
    );
    returned(&mut source.blocks[0]).value = LegalizedScalarReturnValue::Value {
        value: ValueId::new(4).unwrap(),
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
    };
    source.provenance.operations.truncate(4);
    source
}

fn branched_call(target: target::NativeTarget, sibling: bool) -> LegalizedScalarFunction {
    let mut source = derived_call(target, false);
    let mut call = source.blocks[0].instructions.pop().unwrap();
    let slice = source.blocks[0].instructions.pop().unwrap();
    let mut left = source.blocks[0].clone();
    left.id = BlockId::new(2).unwrap();
    left.instructions.clear();
    returned(&mut left).edge = EdgeId::new(2).unwrap();
    returned(&mut left).value = LegalizedScalarReturnValue::Value {
        value: ValueId::new(2).unwrap(),
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
    };
    if sibling {
        left.instructions.push(slice);
    } else {
        source.blocks[0].instructions.push(slice);
    }
    let mut right = source.blocks[0].clone();
    right.id = BlockId::new(3).unwrap();
    call.result.as_mut().unwrap().definition_site = ValueDefinitionSite::Node {
        block: right.id,
        node: 0,
    };
    right.instructions = vec![call];
    returned(&mut right).edge = EdgeId::new(3).unwrap();
    let operation = OperationId::new(5).unwrap();
    let condition = ValueId::new(5).unwrap();
    let node = source.blocks[0].instructions.len() as u32;
    let effect = source.blocks[0].instructions[0].effect;
    source.blocks[0]
        .instructions
        .push(LegalizedScalarInstruction {
            operation,
            result: Some(LegalizedValueDefinition {
                value: condition,
                scalar_type: ScalarType::Boolean,
                definition_site: ValueDefinitionSite::Node {
                    block: source.entry_block,
                    node,
                },
            }),
            kind: LegalizedScalarInstructionKind::Compare {
                predicate: legalized_operations::LegalizedScalarComparison::LessOrEqual,
                operand_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                left: ValueId::new(2).unwrap(),
                right: ValueId::new(1).unwrap(),
            },
            fuel: vec![FuelSettlement {
                site: PsiProvenance::Operation(operation),
                units: 1,
            }],
            effect,
            ownership: Vec::new(),
        });
    let successor = |edge, target| legalized_operations::LegalizedScalarSuccessor {
        edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(target).unwrap(),
        bindings: Vec::new(),
        fuel: Vec::new(),
    };
    source.blocks[0].terminator = LegalizedScalarTerminator::Conditional {
        condition,
        when_true: successor(4, 2),
        when_false: successor(5, 3),
        effect,
        ownership: Vec::new(),
    };
    source.blocks.extend([left, right]);
    source.provenance.operations.push(operation);
    source.provenance.edges = (2..=5).map(|edge| EdgeId::new(edge).unwrap()).collect();
    source
}

#[test]
fn derived_call_requires_actual_descriptor_definition_to_dominate_call_block() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        for sibling in [false, true] {
            let source = branched_call(target, sibling);
            let selected = build(
                0,
                &source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let result = crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                &selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            );
            if sibling {
                assert!(
                    matches!(
                        result,
                        Err(SelectedInstructionError::UseBeforeDefinition { function: 0, .. })
                    ),
                    "only SSA dominance rejects sibling descriptor: {result:?}"
                );
            } else {
                result.unwrap();
            }
        }
    }
}

#[test]
fn derived_call_descriptor_replay_rejects_storage_address_and_fuel_substitution() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        for empty in [false, true] {
            let source = derived_call(target, empty);
            let selected = build(
                0,
                &source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let validate = |source: &LegalizedScalarFunction, selected: &SelectedFunction| {
                crate::selection::validation::scalar_graph::validate(
                    0,
                    source,
                    selected,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            validate(&source, &selected).unwrap();
            let slot = LocalStorageSlotId::Structural {
                operation: OperationId::new(3).unwrap(),
                place: PlaceId::new(2).unwrap(),
            };
            assert_eq!(selected.local_storage_slots.len(), 1);
            assert_eq!(selected.local_storage_slots[0].id, slot);
            assert_eq!(selected.local_storage_slots[0].byte_size, 16);
            assert_eq!(selected.local_storage_slots[0].alignment, 8);
            assert!(selected.outgoing_arguments.is_empty());
            for mutation in 0..10 {
                let mut changed = selected.clone();
                match mutation {
                    0..=2 => {
                        let row = changed.blocks[0].instructions.iter_mut().find(|row| matches!(row.kind, SelectedInstructionKind::Store64 { byte_offset, .. } if byte_offset == if mutation == 1 { 8 } else { 0 })).unwrap();
                        if mutation == 2 {
                            row.kind = SelectedInstructionKind::Store64 {
                                slot: FrameStorageSlotId::Local(slot),
                                byte_offset: 8,
                            };
                        } else {
                            row.operands[0].virtual_register = VirtualRegisterId(0);
                        }
                    }
                    3 => {
                        changed.local_storage_slots[0].id =
                            selected_instructions::LocalStorageSlotId::Structural {
                                operation: OperationId::new(99).unwrap(),
                                place: changed.local_storage_slots[0]
                                    .id
                                    .structural_place()
                                    .unwrap(),
                            }
                    }
                    4 => {
                        changed.local_storage_slots[0].id =
                            selected_instructions::LocalStorageSlotId::Structural {
                                operation: changed.local_storage_slots[0].id.operation(),
                                place: PlaceId::new(99).unwrap(),
                            }
                    }
                    5 => {
                        let row = changed.blocks[0]
                            .instructions
                            .iter_mut()
                            .find(|row| {
                                matches!(row.kind, SelectedInstructionKind::ByteViewAddress)
                            })
                            .unwrap();
                        row.kind = SelectedInstructionKind::ExactAddI64 {
                            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                            accepted_fact:
                                optimization_core::AcceptedObligationFactIdentity::from_bytes(
                                    [1; 32],
                                ),
                        };
                    }
                    6 => changed.blocks[0]
                        .instructions
                        .iter_mut()
                        .find(|row| {
                            matches!(row.kind, SelectedInstructionKind::ExactSubtractI64 { .. })
                        })
                        .unwrap()
                        .provenance
                        .fuel
                        .clear(),
                    7 => changed.blocks[0]
                        .instructions
                        .iter_mut()
                        .find(|row| matches!(row.kind, SelectedInstructionKind::ByteViewAddress))
                        .unwrap()
                        .provenance
                        .fuel
                        .extend(source.blocks[0].instructions[2].fuel.clone()),
                    8 => {
                        let row = changed.blocks[0]
                            .instructions
                            .iter_mut()
                            .find(|row| {
                                matches!(row.kind, SelectedInstructionKind::ByteViewAddress)
                            })
                            .unwrap();
                        row.operands.swap(0, 1);
                    }
                    _ => changed.memory_accesses.last_mut().unwrap().byte_offset += 8,
                }
                assert!(
                    validate(&source, &changed).is_err(),
                    "descriptor mutation {mutation}, empty {empty}, {target:?}"
                );
            }
        }
    }
}
