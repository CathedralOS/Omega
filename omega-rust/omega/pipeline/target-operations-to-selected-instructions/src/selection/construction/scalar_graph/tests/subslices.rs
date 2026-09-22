//! Raw legalization fixtures exercise selection custody, not source admission.
use super::super::{SelectedInstructionError, VirtualRegisterOrigin};
use super::{
    CallSignature, CallingPolicy, FuelSettlement, IntegerSign, IntegerType, IntegerValue,
    LegalizedScalarBlock, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, LegalizedScalarReturn, LegalizedScalarReturnValue,
    LegalizedScalarTerminator, LegalizedValueDefinition, OperationId, PsiProvenance, ScalarType,
    SelectedFunction, SelectedInstructionKind, SelectedSelectionConstraints, StructuralTypeId,
    ValueDefinitionSite, ValueId, ValueShape, VirtualRegisterId, build, evaluate_call_plan,
    fixture, returned,
};
use semantic_vocabulary::{EdgeId, ObligationId, PlaceId};

fn view_result(place: PlaceId) -> terminal_psi::StructuralOperationResult {
    terminal_psi::StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    }
}

pub(super) fn view_fixture(target: target::NativeTarget, empty: bool) -> LegalizedScalarFunction {
    let mut source = fixture(target, 0);
    source.attachment = None;
    let original = PlaceId::new(1).unwrap();
    let suffix = PlaceId::new(2).unwrap();
    let nested = PlaceId::new(3).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![ValueShape::borrowed_reference(16, 8)],
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap();
    source.structural = Some(legalized_operations::LegalizedStructuralContract {
        result: None,
        structural_types: vec![terminal_psi::StructuralTypeDeclaration {
            id: structural_type,
            identity: "bytes".into(),
            shape: terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView,
            ),
        }]
        .into(),
        parameters: vec![legalized_operations::LegalizedCallUnitParameter {
            semantic: terminal_psi::StructuralParameterDeclaration {
                place: original,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                access: terminal_psi::StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
            target: target_operations::TargetStructuralParameter {
                place: original,
                structural_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                access: terminal_psi::StructuralAccess::SharedBorrow,
                projected_qualifications: Vec::new(),
                shape: ValueShape::borrowed_reference(16, 8),
                placement: source.call_plan.parameters[0].clone(),
            },
        }],
        structural_places: vec![terminal_psi::StructuralPlaceDeclaration {
            id: original,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    let value = |raw| ValueId::new(raw).unwrap();
    let fact = optimization_core::AcceptedObligationFactIdentity::from_bytes([1; 32]);
    let mut kinds = vec![
        LegalizedScalarInstructionKind::ByteSequenceLength {
            source: original,
            length_byte_offset: 8,
        },
        LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(1)),
        LegalizedScalarInstructionKind::ByteSequenceSubslice {
            result: view_result(suffix),
            source: original,
            start: value(if empty { 1 } else { 2 }),
            end: value(1),
            length: value(1),
            obligation: ObligationId::new(1).unwrap(),
            accepted_fact: fact,
        },
        LegalizedScalarInstructionKind::ByteSequenceLength {
            source: suffix,
            length_byte_offset: 8,
        },
    ];
    if !empty {
        kinds.extend([
            LegalizedScalarInstructionKind::ByteSequenceSubslice {
                result: view_result(nested),
                source: suffix,
                start: value(2),
                end: value(4),
                length: value(4),
                obligation: ObligationId::new(2).unwrap(),
                accepted_fact: fact,
            },
            LegalizedScalarInstructionKind::ByteSequenceLength {
                source: nested,
                length_byte_offset: 8,
            },
            LegalizedScalarInstructionKind::ByteSequenceRead {
                source: nested,
                index: value(2),
                length: value(6),
                obligation: ObligationId::new(3).unwrap(),
                accepted_fact: fact,
            },
            LegalizedScalarInstructionKind::IntegerWiden {
                operand: value(7),
                source_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            },
        ]);
    }
    let block = &mut source.blocks[0];
    let template = block.instructions[0].clone();
    block.instructions = kinds
        .into_iter()
        .enumerate()
        .map(|(position, kind)| {
            let operation = OperationId::new(position as u64 + 1).unwrap();
            LegalizedScalarInstruction {
                operation,
                result: if matches!(
                    kind,
                    LegalizedScalarInstructionKind::ByteSequenceSubslice { .. }
                ) {
                    None
                } else {
                    Some(LegalizedValueDefinition {
                        value: value(position as u64 + 1),
                        scalar_type: ScalarType::Integer(
                            if matches!(
                                kind,
                                LegalizedScalarInstructionKind::ByteSequenceRead { .. }
                            ) {
                                IntegerType::new(IntegerSign::Unsigned, 8).unwrap()
                            } else {
                                integer
                            },
                        ),
                        definition_site: ValueDefinitionSite::Node {
                            block: block.id,
                            node: position as u32,
                        },
                    })
                },
                kind,
                fuel: vec![FuelSettlement {
                    site: PsiProvenance::Operation(operation),
                    units: 1,
                }],
                effect: template.effect,
                ownership: Vec::new(),
            }
        })
        .collect();
    returned(block).value = LegalizedScalarReturnValue::Value {
        value: value(if empty { 4 } else { 8 }),
        scalar_type: semantic_vocabulary::ScalarType::Integer(integer),
    };
    source.provenance.operations = block.instructions.iter().map(|row| row.operation).collect();
    source
}

#[test]
fn subslice_value_homes_replay_original_empty_and_nested_views() {
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
        for empty in [false, true] {
            let source = view_fixture(target, empty);
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
            if empty {
                assert!(!selected.blocks[0].instructions.iter().any(|row| {
                    matches!(row.kind, SelectedInstructionKind::ExactAddI64 { .. })
                }));
            } else {
                assert_offset_replay(&selected, |candidate| validate(&source, candidate));
            }
            assert!(selected.outgoing_arguments.is_empty());
            let reads = &selected.memory_accesses;
            assert_eq!(
                reads
                    .iter()
                    .filter(|read| read.role
                        == selected_instructions::SelectedMemoryAccessRole::ReadPlace)
                    .count(),
                2
            );
            assert!(
                reads
                    .iter()
                    .filter(|read| read.role
                        == selected_instructions::SelectedMemoryAccessRole::ReadPlace)
                    .all(|read| read.place == PlaceId::new(1).unwrap() && read.byte_count == 8)
            );
            // The payload read names the retained storage root, not the view
            // identity: `nested` is cut from `suffix` cut from `original`, so
            // the byte reach charges `original`'s place with the root bound.
            assert_eq!(
                reads
                    .iter()
                    .filter(|read| matches!(
                        read.role,
                        selected_instructions::SelectedMemoryAccessRole::ReadByteSequence { .. }
                    ))
                    .count(),
                0
            );
            let payload = reads
                .iter()
                .filter(|read| {
                    matches!(
                        read.role,
                        selected_instructions::SelectedMemoryAccessRole::ReadByteSpan { .. }
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(payload.len(), usize::from(!empty));
            for read in payload {
                let selected_instructions::SelectedMemoryAccessRole::ReadByteSpan {
                    length, ..
                } = read.role
                else {
                    unreachable!()
                };
                assert_eq!(read.place, PlaceId::new(1).unwrap());
                assert_eq!(read.byte_offset, 0);
                assert_eq!(read.byte_count, 0);
                assert_eq!(length, ValueId::new(1).unwrap());
            }
            for mutation in 0..11 {
                let mut changed = selected.clone();
                let instruction = changed.blocks[0]
                    .instructions
                    .iter_mut()
                    .find(|row| {
                        matches!(row.kind, SelectedInstructionKind::ExactSubtractI64 { .. })
                    })
                    .unwrap();
                match mutation {
                    0 => instruction.operands.swap(0, 1),
                    1 => instruction.operands[0].virtual_register = VirtualRegisterId(1),
                    2 => {
                        instruction.kind = SelectedInstructionKind::ExactSubtractI64 {
                            obligation: ObligationId::new(999).unwrap(),
                            accepted_fact:
                                optimization_core::AcceptedObligationFactIdentity::from_bytes(
                                    [1; 32],
                                ),
                        }
                    }
                    3 => instruction.provenance.values[0] = ValueId::new(999).unwrap(),
                    4 => instruction.provenance.obligations.clear(),
                    5 => {
                        instruction.kind = SelectedInstructionKind::ExactSubtractI64 {
                            obligation: ObligationId::new(1).unwrap(),
                            accepted_fact:
                                optimization_core::AcceptedObligationFactIdentity::from_bytes(
                                    [2; 32],
                                ),
                        }
                    }
                    6 => {
                        let output = instruction.operands[2].virtual_register;
                        changed.virtual_registers[output.0 as usize].origin =
                            VirtualRegisterOrigin::AbiTransport {
                                instruction: instruction.id,
                                place: PlaceId::new(999).unwrap(),
                                byte_offset: 0,
                            };
                    }
                    7 => {
                        let subtraction = changed.blocks[0]
                            .instructions
                            .iter_mut()
                            .find(|row| {
                                matches!(row.kind, SelectedInstructionKind::ExactSubtractI64 { .. })
                            })
                            .unwrap();
                        subtraction.operands.swap(0, 1);
                    }
                    8 => {
                        let length_read = changed.blocks[0]
                            .instructions
                            .iter_mut()
                            .find(|row| {
                                row.kind == SelectedInstructionKind::CopyI64
                                    && row.provenance.operations
                                        == vec![OperationId::new(4).unwrap()]
                            })
                            .unwrap();
                        length_read.operands[0].virtual_register = VirtualRegisterId(1);
                    }
                    9 => {
                        let subtraction = changed.blocks[0]
                            .instructions
                            .iter_mut()
                            .find(|row| {
                                matches!(row.kind, SelectedInstructionKind::ExactSubtractI64 { .. })
                            })
                            .unwrap();
                        subtraction.provenance.fuel.clear();
                    }
                    _ => {
                        let subtraction = changed.blocks[0]
                            .instructions
                            .iter()
                            .find(|row| {
                                matches!(row.kind, SelectedInstructionKind::ExactSubtractI64 { .. })
                            })
                            .unwrap();
                        let output = subtraction.operands[2].virtual_register;
                        changed.virtual_registers[output.0 as usize].origin =
                            VirtualRegisterOrigin::AbiTransport {
                                instruction: subtraction.id,
                                place: PlaceId::new(2).unwrap(),
                                byte_offset: 0,
                            };
                    }
                }
                assert!(
                    validate(&source, &changed).is_err(),
                    "accepted mutation {mutation} on {target:?}"
                );
            }
            for mutation in 0..6 {
                let mut changed = source.clone();
                let LegalizedScalarInstructionKind::ByteSequenceSubslice {
                    result,
                    source: source_place,
                    start,
                    end,
                    length,
                    obligation,
                    ..
                } = &mut changed.blocks[0].instructions[2].kind
                else {
                    panic!("slice producer")
                };
                match mutation {
                    0 => *source_place = PlaceId::new(999).unwrap(),
                    1 => *start = ValueId::new(999).unwrap(),
                    2 => *end = ValueId::new(2).unwrap(),
                    3 => *length = ValueId::new(2).unwrap(),
                    4 => *obligation = ObligationId::new(999).unwrap(),
                    _ => result.structural_type = StructuralTypeId::new(999).unwrap(),
                }
                assert!(validate(&changed, &selected).is_err());
            }
            if !empty {
                let mut changed = source.clone();
                let LegalizedScalarInstructionKind::ByteSequenceSubslice {
                    source: source_place,
                    ..
                } = &mut changed.blocks[0].instructions[4].kind
                else {
                    panic!("nested producer")
                };
                *source_place = PlaceId::new(1).unwrap();
                assert!(validate(&changed, &selected).is_err());
                let mut changed = selected.clone();
                let read = changed.blocks[0]
                    .instructions
                    .iter_mut()
                    .find(|row| row.kind == SelectedInstructionKind::Load8Indexed)
                    .unwrap();
                read.operands[0].virtual_register = VirtualRegisterId(1);
                assert!(validate(&source, &changed).is_err());
            }
        }
    }
}

fn assert_offset_replay(
    selected: &SelectedFunction,
    validate: impl Fn(&SelectedFunction) -> Result<(), SelectedInstructionError>,
) {
    let instructions = &selected.blocks[0].instructions;
    let backing = instructions
        .iter()
        .find(|row| matches!(row.kind, SelectedInstructionKind::Load64 { byte_offset: 0 }))
        .unwrap()
        .operands[1]
        .virtual_register;
    let root_length = selected
        .virtual_registers
        .iter()
        .find(|register| {
            matches!(register.origin, VirtualRegisterOrigin::InstructionResult { source_value, .. }
            if source_value == ValueId::new(1).unwrap())
        })
        .unwrap()
        .id;
    let start = selected
        .virtual_registers
        .iter()
        .find(|register| {
            matches!(register.origin, VirtualRegisterOrigin::InstructionResult { source_value, .. }
            if source_value == ValueId::new(2).unwrap())
        })
        .unwrap()
        .id;
    let additions = instructions
        .iter()
        .filter(|row| matches!(row.kind, SelectedInstructionKind::ExactAddI64 { .. }))
        .collect::<Vec<_>>();
    let [nested_offset, read_offset] = additions.as_slice() else {
        panic!("nested and read offset sums")
    };
    assert_eq!(
        nested_offset.provenance.operations,
        vec![OperationId::new(5).unwrap()]
    );
    assert_eq!(nested_offset.operands[0].virtual_register, start);
    assert_eq!(nested_offset.operands[1].virtual_register, start);
    assert_eq!(
        read_offset.operands[0].virtual_register,
        nested_offset.operands[2].virtual_register
    );
    assert_eq!(read_offset.operands[1].virtual_register, start);
    let read = instructions
        .iter()
        .find(|row| row.kind == SelectedInstructionKind::Load8Indexed)
        .unwrap();
    assert_eq!(read.operands[0].virtual_register, backing);
    assert_eq!(
        read.operands[1].virtual_register,
        read_offset.operands[2].virtual_register
    );
    for addition in &additions {
        assert!(
            addition.operands[..2]
                .iter()
                .all(|operand| operand.virtual_register != backing)
        );
        assert_eq!(
            addition.provenance.values.last(),
            Some(&ValueId::new(1).unwrap())
        );
        for mutation in 0..8 {
            let mut changed = selected.clone();
            let changed_addition = changed.blocks[0]
                .instructions
                .iter_mut()
                .find(|row| row.id == addition.id)
                .unwrap();
            match mutation {
                0 => changed_addition.operands[0].virtual_register = backing,
                1 => changed_addition.operands[1].virtual_register = root_length,
                2 => {
                    let output = changed_addition.operands[2].virtual_register;
                    let VirtualRegisterOrigin::AbiTransport { byte_offset, .. } =
                        &mut changed.virtual_registers[output.0 as usize].origin
                    else {
                        panic!("offset home")
                    };
                    *byte_offset = 8;
                }
                3 => {
                    let SelectedInstructionKind::ExactAddI64 { obligation, .. } =
                        &mut changed_addition.kind
                    else {
                        panic!("offset addition")
                    };
                    *obligation = ObligationId::new(999).unwrap();
                }
                4 => {
                    let SelectedInstructionKind::ExactAddI64 { accepted_fact, .. } =
                        &mut changed_addition.kind
                    else {
                        panic!("offset addition")
                    };
                    *accepted_fact =
                        optimization_core::AcceptedObligationFactIdentity::from_bytes([9; 32]);
                }
                5 => {
                    *changed_addition.provenance.values.last_mut().unwrap() =
                        ValueId::new(999).unwrap()
                }
                6 => changed_addition.operands[2].virtual_register = start,
                _ => changed_addition.provenance.obligations.clear(),
            }
            assert!(
                validate(&changed).is_err(),
                "accepted offset mutation {mutation}"
            );
        }
    }
    for mutation in 0..5 {
        let mut changed = selected.clone();
        let changed_read = changed.blocks[0]
            .instructions
            .iter_mut()
            .find(|row| row.id == read.id)
            .unwrap();
        match mutation {
            0 => {
                changed_read.operands[0].virtual_register =
                    nested_offset.operands[2].virtual_register
            }
            1 => changed_read.operands[1].virtual_register = start,
            2 => {
                // Forging the backing root: the payload row names `original`.
                let access = changed
                    .memory_accesses
                    .iter_mut()
                    .find(|access| access.instruction == read.id)
                    .unwrap();
                access.place = PlaceId::new(999).unwrap();
            }
            3 => {
                // Reverting to the view identity: `nested` is not the charged
                // storage root and replay rejects the forged distinct place.
                let access = changed
                    .memory_accesses
                    .iter_mut()
                    .find(|access| access.instruction == read.id)
                    .unwrap();
                access.place = PlaceId::new(3).unwrap();
            }
            4 => {
                // Forging the retained extent bound must fail replay.
                let access = changed
                    .memory_accesses
                    .iter_mut()
                    .find(|access| access.instruction == read.id)
                    .unwrap();
                let selected_instructions::SelectedMemoryAccessRole::ReadByteSpan {
                    length, ..
                } = &mut access.role
                else {
                    panic!("logical read footprint")
                };
                *length = ValueId::new(999).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            validate(&changed).is_err(),
            "accepted indexed read mutation {mutation}"
        );
    }
}

/// A byte payload access on a block-parameter view reaches whichever storage
/// every incoming edge binds to the parameter. Both admission and replay
/// enumerate the bound roots and charge the roster rows to them — the view's
/// own identity is never the claimed storage.
fn bound_view_fixture(target: target::NativeTarget) -> LegalizedScalarFunction {
    let mut source = fixture(target, 0);
    source.attachment = None;
    let original = PlaceId::new(1).unwrap();
    let bound = PlaceId::new(2).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let join = semantic_vocabulary::BlockId::new(2).unwrap();
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![ValueShape::borrowed_reference(16, 8)],
            result: Some(ValueShape::integer(1, 1)),
        },
    )
    .unwrap();
    source.structural = Some(legalized_operations::LegalizedStructuralContract {
        result: None,
        structural_types: vec![terminal_psi::StructuralTypeDeclaration {
            id: structural_type,
            identity: "bytes".into(),
            shape: terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView,
            ),
        }]
        .into(),
        parameters: vec![legalized_operations::LegalizedCallUnitParameter {
            semantic: terminal_psi::StructuralParameterDeclaration {
                place: original,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                access: terminal_psi::StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
            target: target_operations::TargetStructuralParameter {
                place: original,
                structural_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                access: terminal_psi::StructuralAccess::MutableBorrow,
                projected_qualifications: Vec::new(),
                shape: ValueShape::borrowed_reference(16, 8),
                placement: source.call_plan.parameters[0].clone(),
            },
        }],
        structural_places: vec![
            terminal_psi::StructuralPlaceDeclaration {
                id: original,
                kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            terminal_psi::StructuralPlaceDeclaration {
                id: bound,
                kind: semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                    block: join,
                    position: 0,
                },
            },
        ],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    let value = |raw| ValueId::new(raw).unwrap();
    let fact = optimization_core::AcceptedObligationFactIdentity::from_bytes([1; 32]);
    let entry = &mut source.blocks[0];
    let effect = entry.instructions[0].effect;
    entry.instructions = vec![LegalizedScalarInstruction {
        operation: OperationId::new(1).unwrap(),
        result: Some(LegalizedValueDefinition {
            value: value(1),
            scalar_type: ScalarType::Integer(integer),
            definition_site: ValueDefinitionSite::Node {
                block: entry.id,
                node: 0,
            },
        }),
        kind: LegalizedScalarInstructionKind::ByteSequenceLength {
            source: original,
            length_byte_offset: 8,
        },
        fuel: vec![FuelSettlement {
            site: PsiProvenance::Operation(OperationId::new(1).unwrap()),
            units: 1,
        }],
        effect,
        ownership: Vec::new(),
    }];
    entry.terminator = LegalizedScalarTerminator::Jump {
        successor: legalized_operations::LegalizedScalarSuccessor {
            edge: EdgeId::new(2).unwrap(),
            target: join,
            bindings: Vec::new(),
            structural_bindings: vec![abstract_operations::AbstractStructuralBinding {
                parameter: bound,
                argument: terminal_psi::StructuralArgument {
                    place: original,
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::MutableBorrow,
                },
            }],
            fuel: Vec::new(),
        },
        effect,
        ownership: Vec::new(),
    };
    let byte = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    let row = |position: u64, kind, result: Option<ValueId>, scalar_type| {
        let operation = OperationId::new(position).unwrap();
        LegalizedScalarInstruction {
            operation,
            result: result.map(|value| LegalizedValueDefinition {
                value,
                scalar_type,
                definition_site: ValueDefinitionSite::Node {
                    block: join,
                    node: position as u32 - 2,
                },
            }),
            kind,
            fuel: vec![FuelSettlement {
                site: PsiProvenance::Operation(operation),
                units: 1,
            }],
            effect,
            ownership: Vec::new(),
        }
    };
    source.blocks.push(LegalizedScalarBlock {
        structural_parameters: vec![terminal_psi::StructuralParameterDeclaration {
            place: bound,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            access: terminal_psi::StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        id: join,
        parameters: Vec::new(),
        instructions: vec![
            row(
                2,
                LegalizedScalarInstructionKind::ByteSequenceLength {
                    source: bound,
                    length_byte_offset: 8,
                },
                Some(value(2)),
                ScalarType::Integer(integer),
            ),
            row(
                3,
                LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(0)),
                Some(value(3)),
                ScalarType::Integer(integer),
            ),
            row(
                4,
                LegalizedScalarInstructionKind::ByteSequenceRead {
                    source: bound,
                    index: value(3),
                    length: value(2),
                    obligation: ObligationId::new(1).unwrap(),
                    accepted_fact: fact,
                },
                Some(value(4)),
                byte,
            ),
            row(
                5,
                LegalizedScalarInstructionKind::ByteSequenceWrite {
                    destination: bound,
                    index: value(3),
                    value: value(4),
                    length: value(2),
                    obligation: ObligationId::new(2).unwrap(),
                    accepted_fact: fact,
                },
                None,
                byte,
            ),
        ],
        terminator: LegalizedScalarTerminator::Return(LegalizedScalarReturn {
            edge: EdgeId::new(3).unwrap(),
            value: LegalizedScalarReturnValue::Value {
                value: value(4),
                scalar_type: byte,
            },
            fuel: Vec::new(),
            effect,
            ownership: Vec::new(),
        }),
    });
    source.provenance.operations = (1..=5).map(|raw| OperationId::new(raw).unwrap()).collect();
    source.provenance.edges = (1..=3).map(|raw| EdgeId::new(raw).unwrap()).collect();
    source
}

#[test]
fn block_parameter_view_payloads_charge_the_bound_storage_roots() {
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
        let source = bound_view_fixture(target);
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
        let original = PlaceId::new(1).unwrap();
        let bound = PlaceId::new(2).unwrap();
        let length = ValueId::new(2).unwrap();
        let read = selected
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| row.kind == SelectedInstructionKind::Load8Indexed)
            .unwrap();
        let write = selected
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| {
                row.kind
                    == SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 1,
                    }
            })
            .unwrap();
        for (instruction, role) in [(read.id, "ReadByteSpan"), (write.id, "WriteByteSpan")] {
            let rows = selected
                .memory_accesses
                .iter()
                .filter(|access| access.instruction == instruction)
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), 1, "{role} rows for {target:?}");
            let span = match rows[0].role {
                selected_instructions::SelectedMemoryAccessRole::ReadByteSpan {
                    length: extent,
                    ..
                } if role == "ReadByteSpan" => extent,
                selected_instructions::SelectedMemoryAccessRole::WriteByteSpan {
                    length: extent,
                    ..
                } if role == "WriteByteSpan" => extent,
                _ => panic!("{role} row for {target:?}"),
            };
            assert_eq!(rows[0].place, original);
            assert_eq!(rows[0].byte_offset, 0);
            assert_eq!(rows[0].byte_count, 0);
            assert_eq!(span, length);
        }
        // No row keeps charging the view's own identity.
        assert!(!selected.memory_accesses.iter().any(|access| matches!(
            access.role,
            selected_instructions::SelectedMemoryAccessRole::ReadByteSequence { .. }
                | selected_instructions::SelectedMemoryAccessRole::WriteByteSequence { .. }
        )));
        for mutation in 0..6 {
            let mut changed = selected.clone();
            let (instruction, write_side) = match mutation {
                0..=2 => (read.id, false),
                _ => (write.id, true),
            };
            let access = changed
                .memory_accesses
                .iter_mut()
                .find(|access| access.instruction == instruction)
                .unwrap();
            match mutation % 3 {
                // A forged distinct root must fail replay.
                0 => access.place = PlaceId::new(999).unwrap(),
                // Reverting to the view's own identity must fail replay.
                1 => access.place = bound,
                _ => {
                    // A forged extent bound must fail replay.
                    let span = if write_side {
                        match &mut access.role {
                            selected_instructions::SelectedMemoryAccessRole::WriteByteSpan {
                                length,
                                ..
                            } => length,
                            _ => panic!("write span"),
                        }
                    } else {
                        match &mut access.role {
                            selected_instructions::SelectedMemoryAccessRole::ReadByteSpan {
                                length,
                                ..
                            } => length,
                            _ => panic!("read span"),
                        }
                    };
                    *span = ValueId::new(999).unwrap();
                }
            }
            assert!(
                validate(&source, &changed).is_err(),
                "accepted bound-root mutation {mutation} on {target:?}"
            );
        }
    }
}
