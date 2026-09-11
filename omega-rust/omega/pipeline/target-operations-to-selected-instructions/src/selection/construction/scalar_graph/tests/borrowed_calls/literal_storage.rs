//! Local literal bytes and descriptor storage are replayed from one source producer.
use super::*;
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, SelectedMemoryAccessRole};

fn literal_call(target: target::NativeTarget, bytes: &[u8]) -> LegalizedScalarFunction {
    let mut source = borrowed_call(target);
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: Vec::new(),
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap();
    let signature = source.structural.as_mut().unwrap();
    signature.parameters.clear();
    let destination = &mut signature.structural_places[0];
    destination.kind = StructuralPlaceKind::ByteSequenceLiteral {
        declaration_ordinal: 0,
        structural_type: signature.structural_types[0].id,
    };
    let operation = OperationId::new(20).unwrap();
    let row = &mut source.blocks[0].instructions[0];
    row.result.as_mut().unwrap().definition_site = ValueDefinitionSite::Node {
        block: source.entry_block,
        node: 1,
    };
    row.effect = EffectLink {
        input: 1,
        output: 2,
    };
    let LegalizedScalarInstructionKind::Call(call) = &mut row.kind else {
        panic!("call fixture");
    };
    let LegalizedScalarArgument::Structural { target, .. } = &mut call.arguments[0] else {
        panic!("view argument");
    };
    target.source = target_operations::TargetStructuralArgumentSource::EstablishedByteView {
        psi_operation: operation,
    };
    let establishment = LegalizedScalarInstruction {
        operation,
        result: None,
        kind: LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
            destination: *destination,
            structural_type: signature.structural_types[0].clone(),
            bytes: bytes.to_vec(),
        },
        fuel: vec![FuelSettlement {
            site: PsiProvenance::Operation(operation),
            units: 1,
        }],
        effect: EffectLink {
            input: 0,
            output: 1,
        },
        ownership: Vec::new(),
    };
    source.blocks[0].instructions.insert(0, establishment);
    source.provenance.operations.insert(0, operation);
    returned(&mut source.blocks[0]).effect = EffectLink {
        input: 2,
        output: 3,
    };
    source
}

#[test]
fn literal_storage_replays_raw_bytes_descriptor_geometry_and_single_fuel() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        for bytes in [
            &[][..],
            &[0, 128, 255][..],
            &[1, 2, 3, 4, 5, 6, 7, 8, 255][..],
        ] {
            let source = literal_call(target, bytes);
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
            let operation = source.blocks[0].instructions[0].operation;
            let place = source.structural.as_ref().unwrap().structural_places[0].id;
            let slot = LocalStorageSlotId::Structural { operation, place };
            assert_eq!(selected.local_storage_slots.len(), 1);
            assert_eq!(selected.local_storage_slots[0].id, slot);
            assert_eq!(
                selected.local_storage_slots[0].byte_size,
                16 + (bytes.len() as u32).div_ceil(8) * 8
            );
            assert_eq!(selected.local_storage_slots[0].alignment, 8);
            assert!(selected.outgoing_arguments.is_empty());
            let backing = selected
                .memory_accesses
                .iter()
                .find(|access| {
                    access.byte_offset == 16
                        && matches!(access.role, SelectedMemoryAccessRole::AddressLocal { .. })
                })
                .unwrap();
            assert_eq!(backing.byte_count, bytes.len() as u32);
            assert_eq!(
                backing.role,
                SelectedMemoryAccessRole::AddressLocal { slot }
            );
            let settled = selected.blocks[0]
                .instructions
                .iter()
                .flat_map(|row| &row.provenance.fuel)
                .filter(|fuel| fuel.site == PsiProvenance::Operation(operation))
                .collect::<Vec<_>>();
            assert_eq!(
                settled,
                source.blocks[0].instructions[0]
                    .fuel
                    .iter()
                    .collect::<Vec<_>>()
            );

            for mutation in 0..10 {
                let mut changed = selected.clone();
                match mutation {
                    0 => {
                        changed.local_storage_slots[0].id =
                            selected_instructions::LocalStorageSlotId::Structural {
                                operation: OperationId::new(99).unwrap(),
                                place: changed.local_storage_slots[0]
                                    .id
                                    .structural_place()
                                    .unwrap(),
                            }
                    }
                    1 => {
                        changed.local_storage_slots[0].id =
                            selected_instructions::LocalStorageSlotId::Structural {
                                operation: changed.local_storage_slots[0]
                                    .id
                                    .operation()
                                    .expect("source-backed local slot"),
                                place: PlaceId::new(99).unwrap(),
                            }
                    }
                    2 => changed.local_storage_slots[0].byte_size += 8,
                    3 => changed.local_storage_slots[0].alignment = 16,
                    4 => {
                        let access = changed
                            .memory_accesses
                            .iter_mut()
                            .find(|access| {
                                access.byte_offset == 16
                                    && matches!(
                                        access.role,
                                        SelectedMemoryAccessRole::AddressLocal { .. }
                                    )
                            })
                            .unwrap();
                        access.byte_count += 1;
                    }
                    5 => changed.memory_accesses[0].place = PlaceId::new(99).unwrap(),
                    6 => {
                        let row = changed.blocks[0]
                            .instructions
                            .iter_mut()
                            .find(|row| {
                                matches!(
                                    row.kind,
                                    SelectedInstructionKind::FrameAddress { byte_offset: 0, .. }
                                )
                            })
                            .unwrap();
                        row.provenance.fuel.clear();
                    }
                    7 => changed.blocks[0].instructions[0]
                        .provenance
                        .fuel
                        .extend(source.blocks[0].instructions[0].fuel.clone()),
                    8 => {
                        let row = changed.blocks[0]
                            .instructions
                            .iter_mut()
                            .find(|row| {
                                matches!(
                                    row.kind,
                                    SelectedInstructionKind::Store64 { byte_offset: 8, .. }
                                )
                            })
                            .unwrap();
                        row.kind = SelectedInstructionKind::Store64 {
                            slot: FrameStorageSlotId::Local(slot),
                            byte_offset: 0,
                        };
                    }
                    _ => {
                        let row = changed.blocks[0]
                            .instructions
                            .iter_mut()
                            .find(|row| {
                                matches!(row.kind, SelectedInstructionKind::MaterializeI64 { .. })
                            })
                            .unwrap();
                        let SelectedInstructionKind::MaterializeI64 { value } = &mut row.kind
                        else {
                            unreachable!()
                        };
                        let IntegerValue::Unsigned(raw) = value else {
                            unreachable!()
                        };
                        *raw ^= 1_u128 << 56;
                    }
                }
                assert!(
                    validate(&source, &changed).is_err(),
                    "selected literal mutation {mutation}, length {}",
                    bytes.len()
                );
            }
            for mutation in 0..4 {
                let mut changed = source.clone();
                if mutation == 3 {
                    let LegalizedScalarInstructionKind::Call(call) =
                        &mut changed.blocks[0].instructions[1].kind
                    else {
                        unreachable!()
                    };
                    let LegalizedScalarArgument::Structural { target, .. } = &mut call.arguments[0]
                    else {
                        unreachable!()
                    };
                    target.source =
                        target_operations::TargetStructuralArgumentSource::EstablishedByteView {
                            psi_operation: OperationId::new(99).unwrap(),
                        };
                } else {
                    let LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                        destination,
                        bytes,
                        ..
                    } = &mut changed.blocks[0].instructions[0].kind
                    else {
                        unreachable!()
                    };
                    match mutation {
                        0 => bytes.push(0),
                        1 => {
                            if let Some(byte) = bytes.first_mut() {
                                *byte ^= 128;
                            } else {
                                bytes.push(128);
                            }
                        }
                        _ => destination.id = PlaceId::new(99).unwrap(),
                    }
                }
                assert!(
                    validate(&changed, &selected).is_err(),
                    "literal source mutation {mutation}"
                );
            }
            for mutation in 0..4 {
                let mut changed = source.clone();
                match mutation {
                    0 => changed
                        .structural
                        .as_mut()
                        .unwrap()
                        .structural_places
                        .clear(),
                    1 => changed.blocks[0].instructions.swap(0, 1),
                    2 => changed
                        .structural
                        .as_mut()
                        .unwrap()
                        .structural_types
                        .make_mut()
                        .clear(),
                    _ => {
                        changed.structural.as_mut().unwrap().structural_places[0].kind =
                            StructuralPlaceKind::ByteSequenceLiteral {
                                declaration_ordinal: 1,
                                structural_type: StructuralTypeId::new(1).unwrap(),
                            }
                    }
                }
                assert!(
                    build(
                        0,
                        &changed,
                        target,
                        &constraints,
                        environment.physical(),
                        environment.constraints()
                    )
                    .is_err(),
                    "literal roster mutation {mutation}"
                );
                assert!(
                    validate(&changed, &selected).is_err(),
                    "literal roster replay mutation {mutation}"
                );
            }
        }
    }
}
