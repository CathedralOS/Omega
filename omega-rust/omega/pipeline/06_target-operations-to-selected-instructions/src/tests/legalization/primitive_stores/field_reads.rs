//! Field identities survive independent once-only scalar observations.
use super::{
    AbstractOperation, AbstractResult, BindingRelevance, FuelScheduleIdentity, IntegerSign,
    LegalizedScalarInstructionKind, NativeTarget, OperationId, PlaceId, ScalarType,
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldId, StructuralFieldType,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, TargetUnitOperation, ValueId,
    fixture,
};
use crate::legalize_target_operations;
use crate::tests::legalization::primitive_stores::integer;
use crate::validate_legalized_operations;
use abstract_operations::AbstractFunctionResult;
use semantic_vocabulary::EdgeId;

mod indirect_inputs;

#[test]
fn field_observations_replay_parameter_field_offset_and_result() {
    field_observations(StructuralAccess::SharedBorrow, false, false);
}

#[test]
fn owned_field_observations_replay_value_abi_home_and_exact_initialization() {
    field_observations(StructuralAccess::Owned, false, false);
}

#[test]
fn nested_field_observations_replay_declaration_local_ids_and_original_root() {
    field_observations(StructuralAccess::SharedBorrow, true, false);
}

#[test]
fn byte_field_metadata_replays_exact_subject_without_content_read_authority() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        for nested in [false, true] {
            field_observations(access, nested, true);
        }
    }
}

/// A terminal literal `FixedIndex` ends the bounded carrier of a scalar field
/// observation (`self.maps[1].value`): the element record owns the observed
/// field. Legalization rechecks the index against the declared extent and
/// re-emits the exact runtime path lowering produced.
#[test]
fn indexed_field_observations_replay_literal_element_offset() {
    use semantic_vocabulary::CanonicalStructuralPathSegment as Segment;
    let scalar = integer(IntegerSign::Signed, 32);
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for access in [
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
        ] {
            let (mut source, _, _) = fixture(native, scalar, false);
            let element = StructuralTypeId::new(2).unwrap();
            let array = StructuralTypeId::new(3).unwrap();
            let maps = StructuralFieldId::new(2).unwrap();
            let value = StructuralFieldId::new(1).unwrap();
            source.structural_types.make_mut()[0].shape = StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "padding".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(scalar),
                    },
                    StructuralFieldDeclaration {
                        id: maps,
                        identity: "maps".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(array),
                    },
                ],
            };
            let mut types = source.structural_types.to_vec();
            types.extend([
                StructuralTypeDeclaration {
                    id: element,
                    identity: "Map".into(),
                    shape: StructuralTypeShape::Record {
                        fields: vec![StructuralFieldDeclaration {
                            id: value,
                            identity: "value".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(scalar),
                        }],
                    },
                },
                StructuralTypeDeclaration {
                    id: array,
                    identity: "Maps".into(),
                    shape: StructuralTypeShape::FixedArray { element, length: 2 },
                },
            ]);
            source.structural_types = types.into();
            let function = &mut source.functions[0];
            function.parameters.clear();
            function.structural_parameters[0].access = access;
            let parameter = function.structural_parameters[0].clone();
            let read = AbstractResult {
                value: ValueId::new(7).unwrap(),
                scalar_type: scalar,
            };
            let result = AbstractResult {
                value: ValueId::new(8).unwrap(),
                scalar_type: scalar,
            };
            function.result = AbstractFunctionResult::Scalar(result);
            function.operations = vec![
                AbstractOperation::IntegerStructuralField {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: read,
                    path: vec![Segment::Field(maps), Segment::FixedIndex(1)],
                    source: parameter.place,
                    field: value,
                },
                AbstractOperation::Return {
                    psi_edge: EdgeId::new(1).unwrap(),
                    result: result.value,
                    value: read.value,
                    scalar_type: scalar,
                    cleanup_actions: Vec::new(),
                },
            ];
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
                .unwrap();
            // The legalized observation keeps lowering's exact runtime shape:
            // the field identity followed by the literal element index.
            let LegalizedScalarInstructionKind::StructuralScalarFieldRead {
                source: argument, ..
            } = &legalized.plan().scalar_functions[0].blocks[0].instructions[0].kind
            else {
                panic!("field read");
            };
            assert_eq!(
                argument.path,
                vec![
                    terminal_psi::StructuralPathSegment::Field("maps".into()),
                    terminal_psi::StructuralPathSegment::FixedIndex(1),
                ]
            );
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legalized, &environment);
            let selected = crate::select_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            // The four-byte element stride places `maps[1].value` at byte 8:
            // `padding` owns bytes 0..4 and `maps[0].value` owns 4..8.
            let load = selected.plan().functions[0].blocks[0]
                .instructions
                .iter()
                .find(|instruction| {
                    matches!(
                        instruction.kind,
                        selected_instructions::SelectedInstructionKind::Load32 { .. }
                    )
                })
                .expect("indexed element load");
            let selected_instructions::SelectedInstructionKind::Load32 { byte_offset } = load.kind
            else {
                unreachable!()
            };
            assert_eq!(byte_offset, 8);
            // A retained path that drops, moves, or substitutes the literal
            // element index no longer names this observation's subject.
            for path in [
                Vec::new(),
                vec![terminal_psi::StructuralPathSegment::Field("maps".into())],
                vec![
                    terminal_psi::StructuralPathSegment::Field("maps".into()),
                    terminal_psi::StructuralPathSegment::FixedIndex(0),
                ],
                vec![
                    terminal_psi::StructuralPathSegment::Field("maps".into()),
                    terminal_psi::StructuralPathSegment::Field("1".into()),
                ],
                vec![terminal_psi::StructuralPathSegment::FixedIndex(1)],
            ] {
                let mut changed = target.clone();
                let TargetUnitOperation::StructuralScalarFieldRead {
                    source: argument, ..
                } = &mut changed.functions[0].graph.blocks[0].operations[0]
                else {
                    panic!("field read");
                };
                argument.path = path;
                super::reject_target(&source, &changed, &unit, legalized.plan());
            }
            // Malformed canonical carriers stay fail-closed at the source
            // boundary: out of extent, index into a record, mid-path index,
            // and a case segment.
            for mutation in [
                vec![Segment::Field(maps), Segment::FixedIndex(2)],
                vec![Segment::FixedIndex(0)],
                vec![
                    Segment::Field(maps),
                    Segment::FixedIndex(0),
                    Segment::Field(value),
                ],
                vec![
                    Segment::Field(maps),
                    Segment::Case(semantic_vocabulary::StructuralCaseId::new(1).unwrap()),
                ],
            ] {
                let mut changed = source.clone();
                let AbstractOperation::IntegerStructuralField { path, .. } =
                    &mut changed.functions[0].operations[0]
                else {
                    panic!("field read");
                };
                *path = mutation;
                assert!(
                    abstract_operations_to_target_operations::lower_to_target_operations(
                        &changed,
                        abstract_operations_to_target_operations::TargetLoweringRequest::new(
                            native
                        )
                    )
                    .is_err()
                );
            }
            // The same malformed runtime carriers stay closed in the legalized
            // plan's independent replay.
            for path in [
                vec![
                    terminal_psi::StructuralPathSegment::Field("maps".into()),
                    terminal_psi::StructuralPathSegment::FixedIndex(2),
                ],
                vec![
                    terminal_psi::StructuralPathSegment::Field("maps".into()),
                    terminal_psi::StructuralPathSegment::FixedIndex(0),
                    terminal_psi::StructuralPathSegment::Field("value".into()),
                ],
            ] {
                let mut changed = legalized.plan().clone();
                let LegalizedScalarInstructionKind::StructuralScalarFieldRead {
                    source: argument,
                    ..
                } = &mut changed.scalar_functions[0].blocks[0].instructions[0].kind
                else {
                    panic!("field read");
                };
                argument.path = path;
                assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
            }
        }
    }
}

fn field_observations(access: StructuralAccess, nested: bool, byte_length: bool) {
    use semantic_vocabulary::{BoundedIntegerType, IntegerType, IntegerValue};

    let mut field_types = vec![
        StructuralFieldType::Scalar(integer(IntegerSign::Signed, 8)),
        StructuralFieldType::Scalar(integer(IntegerSign::Unsigned, 64)),
        StructuralFieldType::Scalar(ScalarType::Boolean),
        StructuralFieldType::BoundedInteger(
            BoundedIntegerType::new(
                IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                IntegerValue::Signed(-3),
                IntegerValue::Signed(3),
            )
            .unwrap(),
        ),
        StructuralFieldType::BoundedInteger(
            BoundedIntegerType::new(
                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                IntegerValue::Unsigned(1_u128 << 63),
                IntegerValue::Unsigned(u128::from(u64::MAX)),
            )
            .unwrap(),
        ),
    ];
    if byte_length {
        field_types = vec![StructuralFieldType::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 3 },
        )];
    }
    for field_type in field_types {
        let scalar = if byte_length {
            integer(IntegerSign::Unsigned, 64)
        } else {
            field_type.scalar_type().unwrap()
        };
        for native in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let (mut source, _, _) = fixture(native, scalar, false);
            let declaration = &mut source.structural_types.make_mut()[0];
            declaration.shape = StructuralTypeShape::Record {
                fields: [1, 2]
                    .map(|ordinal| StructuralFieldDeclaration {
                        id: StructuralFieldId::new(ordinal).unwrap(),
                        identity: format!("field{ordinal}"),
                        relevance: BindingRelevance::Relevant,
                        field_type: field_type.clone(),
                    })
                    .to_vec(),
            };
            let path = if nested {
                let mut child = declaration.clone();
                child.id = StructuralTypeId::new(99).unwrap();
                child.identity = "NestedReadCarrier".into();
                declaration.shape = StructuralTypeShape::Record {
                    fields: vec![
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(1).unwrap(),
                            identity: "padding".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(scalar),
                        },
                        StructuralFieldDeclaration {
                            id: StructuralFieldId::new(2).unwrap(),
                            identity: "nested".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(child.id),
                        },
                    ],
                };
                let mut types = source.structural_types.to_vec();
                types.push(child);
                source.structural_types = types.into();
                vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    StructuralFieldId::new(2).unwrap(),
                )]
            } else {
                Vec::new()
            };
            let function = &mut source.functions[0];
            function.parameters.clear();
            function.structural_parameters[0].access = access;
            let parameter = function.structural_parameters[0].clone();
            let read = |ordinal| {
                let psi_operation = OperationId::new(ordinal).unwrap();
                let result = ValueId::new(ordinal).unwrap();
                let field = StructuralFieldId::new(ordinal).unwrap();
                if byte_length {
                    AbstractOperation::StructuralByteSequenceFieldLength {
                        psi_operation,
                        result: AbstractResult {
                            value: result,
                            scalar_type: scalar,
                        },
                        source: parameter.place,
                        path: if nested {
                            vec![terminal_psi::StructuralPathSegment::Field("nested".into())]
                        } else {
                            Vec::new()
                        },
                        field,
                    }
                } else if scalar == ScalarType::Boolean {
                    AbstractOperation::BooleanStructuralField {
                        psi_operation,
                        result,
                        path: path.clone(),
                        source: parameter.place,
                        field,
                    }
                } else {
                    AbstractOperation::IntegerStructuralField {
                        psi_operation,
                        result: AbstractResult {
                            value: result,
                            scalar_type: scalar,
                        },
                        path: path.clone(),
                        source: parameter.place,
                        field,
                    }
                }
            };
            let result = AbstractResult {
                value: ValueId::new(3).unwrap(),
                scalar_type: scalar,
            };
            function.result = AbstractFunctionResult::Scalar(result);
            function.operations = vec![
                read(1),
                read(2),
                AbstractOperation::Return {
                    psi_edge: EdgeId::new(1).unwrap(),
                    result: result.value,
                    value: ValueId::new(2).unwrap(),
                    scalar_type: scalar,
                    cleanup_actions: Vec::new(),
                },
            ];
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            optimization_unit_semantics::validate_psi_optimization_unit(&unit)
                .unwrap_or_else(|error| panic!("{scalar:?} optimizer: {error:?}"));
            let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
            if byte_length {
                // Equal root/field/result payloads cannot exchange metadata
                // observation for a read of contents at either boundary.
                let mut changed = target.clone();
                let TargetUnitOperation::StructuralByteSequenceFieldLength {
                    psi_operation,
                    result,
                    source: argument,
                    field,
                } = changed.functions[0].graph.blocks[0].operations[1].clone()
                else {
                    panic!("metadata read");
                };
                changed.functions[0].graph.blocks[0].operations[1] =
                    TargetUnitOperation::StructuralScalarFieldRead {
                        psi_operation,
                        result,
                        source: argument.clone(),
                        field,
                    };
                assert!(legalize_target_operations(&changed, &source, &unit).is_err());
                let mut changed = legalized.plan().clone();
                changed.scalar_functions[0].blocks[0].instructions[1].kind =
                    LegalizedScalarInstructionKind::StructuralScalarFieldRead {
                        source: argument,
                        field,
                    };
                assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
            }
            validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
                .unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legalized, &environment);
            let selected = crate::select_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let indirect_owned = access == StructuralAccess::Owned
                && matches!(
                    legalized.plan().scalar_functions[0].call_plan.parameters[0]
                        .locations
                        .as_slice(),
                    [calling_conventions::ValueLocation::Indirect { .. }]
                );
            let captured_fragments = if access == StructuralAccess::Owned && !indirect_owned {
                1 + legalized.plan().scalar_functions[0].call_plan.parameters[0]
                    .locations
                    .len()
            } else {
                0
            };
            assert_eq!(
                selected.plan().functions[0].memory_accesses.len(),
                2 + captured_fragments
            );
            if indirect_owned {
                assert!(selected.plan().functions[0].local_storage_slots.is_empty());
            }
            if access == StructuralAccess::Owned && !indirect_owned {
                use selected_instructions::{
                    LocalStorageSlotId, SelectedInstructionKind as Instruction,
                };
                let retained = &selected.plan().functions[0];
                assert_eq!(retained.local_storage_slots.len(), 1);
                assert_eq!(
                    retained.local_storage_slots[0].id,
                    LocalStorageSlotId::StructuralParameter {
                        place: parameter.place
                    }
                );
                let incoming = retained
                    .virtual_registers
                    .iter()
                    .find(|register| {
                        matches!(
                            register.origin,
                            selected_instructions::VirtualRegisterOrigin::StructuralParameter { .. }
                        )
                    })
                    .unwrap()
                    .id;
                for mutation in [
                    "extent",
                    "slot owner",
                    "initialization width",
                    "missing initialization",
                    "value as pointer",
                ] {
                    let mut changed = selected.plan().clone();
                    let function = &mut changed.functions[0];
                    match mutation {
                        "extent" => function.local_storage_slots[0].byte_size += 1,
                        "slot owner" => {
                            function.local_storage_slots[0].id =
                                LocalStorageSlotId::StructuralParameter {
                                    place: PlaceId::new(99).unwrap(),
                                }
                        }
                        "initialization width" | "missing initialization" => {
                            let store = function.blocks[0]
                                .instructions
                                .iter_mut()
                                .find(|row| matches!(row.kind, Instruction::Store { .. }))
                                .unwrap();
                            if mutation == "missing initialization" {
                                store.kind = Instruction::CopyI64;
                            } else if let Instruction::Store { byte_size, .. } = &mut store.kind {
                                *byte_size += 1;
                            }
                        }
                        _ => {
                            let read = function.blocks[0]
                                .instructions
                                .iter_mut()
                                .find(|row| {
                                    matches!(
                                        row.kind,
                                        Instruction::Load8 { .. } | Instruction::Load64 { .. }
                                    )
                                })
                                .unwrap();
                            read.operands[0].virtual_register = incoming;
                        }
                    }
                    assert!(
                        crate::validate_selected_instructions(
                            &legalized,
                            &constraints,
                            environment.physical(),
                            environment.constraints(),
                            changed
                        )
                        .is_err(),
                        "{native:?} {scalar:?} {mutation}"
                    );
                }
            }
            let mut changed = selected.plan().clone();
            let instruction = changed.functions[0].blocks[0]
                .instructions
                .iter_mut()
                .find(|instruction| {
                    instruction.provenance.operations == vec![OperationId::new(2).unwrap()]
                        && matches!(
                            instruction.kind,
                            selected_instructions::SelectedInstructionKind::Load8 { .. }
                                | selected_instructions::SelectedInstructionKind::Load64 { .. }
                        )
                })
                .expect("the second observation has its own exact load");
            match &mut instruction.kind {
                selected_instructions::SelectedInstructionKind::Load8 { byte_offset }
                | selected_instructions::SelectedInstructionKind::Load64 { byte_offset } => {
                    *byte_offset = 0
                }
                _ => unreachable!(),
            }
            assert!(
                crate::validate_selected_instructions(
                    &legalized,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed
                )
                .is_err()
            );
            for mutation in ["field", "access", "source", "result", "path"] {
                let mut changed = target.clone();
                let (TargetUnitOperation::StructuralScalarFieldRead {
                    field,
                    source: argument,
                    result,
                    ..
                }
                | TargetUnitOperation::StructuralByteSequenceFieldLength {
                    field,
                    source: argument,
                    result,
                    ..
                }) = &mut changed.functions[0].graph.blocks[0].operations[1]
                else {
                    panic!("field observation");
                };
                match mutation {
                    "field" => *field = StructuralFieldId::new(1).unwrap(),
                    "access" => {
                        argument.access = if access == StructuralAccess::MutableBorrow {
                            StructuralAccess::SharedBorrow
                        } else {
                            StructuralAccess::MutableBorrow
                        }
                    }
                    "source" => argument.place = PlaceId::new(99).unwrap(),
                    "result" => result.value = ValueId::new(1).unwrap(),
                    "path" => {
                        if nested {
                            argument.path.clear();
                        } else {
                            argument
                                .path
                                .push(terminal_psi::StructuralPathSegment::Field(
                                    "invented".into(),
                                ));
                        }
                    }
                    _ => unreachable!(),
                }
                assert!(
                    legalize_target_operations(&changed, &source, &unit).is_err(),
                    "{native:?} {scalar:?} {mutation}"
                );
                assert!(
                    validate_legalized_operations(
                        &changed,
                        &source,
                        &unit,
                        legalized.plan().clone()
                    )
                    .is_err(),
                    "{native:?} {scalar:?} {mutation}"
                );
            }
            let mut changed = legalized.plan().clone();
            let (LegalizedScalarInstructionKind::StructuralScalarFieldRead { field, .. }
            | LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength {
                field, ..
            }) = &mut changed.scalar_functions[0].blocks[0].instructions[1].kind
            else {
                panic!("field row");
            };
            *field = StructuralFieldId::new(1).unwrap();
            assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
        }
    }
}
