//! Store replay binds the original pointer, exact write width, SSA source and charge.

use super::{
    CallSignature, CallingPolicy, IntegerSign, IntegerType, LegalizedScalarInstructionKind,
    ScalarType, SelectedFunction, SelectedInstructionKind, SelectedSelectionConstraints, ValueId,
    ValueShape, build, evaluate_call_plan, fixture,
};

#[test]
fn established_reference_primitive_store_replays_the_pointer_load() {
    use abstract_operations::{AbstractResult, StructuralScalarStoreDestination};
    use selected_instructions::{SelectedMemoryAccessOrigin, SelectedMemoryAccessRole};
    use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind, StructuralTypeId};
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
        StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    let target = target::NativeTarget::macos_arm64();
    let environment = register_environment::baseline_target_register_environment(target).unwrap();
    let mut source = fixture(target, 0);
    source.attachment = None;
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let primitive_type = StructuralTypeId::new(1).unwrap();
    let reference_type = StructuralTypeId::new(2).unwrap();
    let backing = PlaceId::new(1).unwrap();
    let carrier = PlaceId::new(2).unwrap();
    let producer = OperationId::new(2).unwrap();
    let store = OperationId::new(3).unwrap();
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![ValueShape::borrowed_reference(8, 8)],
            result: None,
        },
    )
    .unwrap();
    let parameter = StructuralParameterDeclaration {
        place: backing,
        position: 0,
        is_self: false,
        structural_type: primitive_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: vec![],
        projected_qualifications: vec![],
    };
    let reference = StructuralOperationResult {
        place: carrier,
        structural_type: reference_type,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: vec![],
        projected_qualifications: vec![],
        qualification_establishments: vec![],
        claims: vec![],
    };
    source.structural = Some(legalized_operations::LegalizedStructuralContract {
        result: None,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: primitive_type,
                identity: "primitive".into(),
                shape: StructuralTypeShape::PrimitiveScalar(scalar),
            },
            StructuralTypeDeclaration {
                id: reference_type,
                identity: "reference".into(),
                shape: StructuralTypeShape::Reference {
                    referent: primitive_type,
                    access: StructuralAccess::MutableBorrow,
                },
            },
        ]
        .into(),
        parameters: vec![legalized_operations::LegalizedCallUnitParameter {
            semantic: parameter.clone(),
            target: target_operations::TargetStructuralParameter {
                place: backing,
                structural_type: primitive_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                projected_qualifications: vec![],
                shape: source.call_plan.parameters[0].shape,
                placement: source.call_plan.parameters[0].clone(),
            },
        }],
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: backing,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: carrier,
                kind: StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type: reference_type,
                },
            },
        ],
        entry_claims: vec![],
        published_service_ceiling: vec![],
    });
    for instruction in &mut source.blocks[0].instructions[1..] {
        instruction.result = None;
    }
    source.blocks[0].instructions[1].kind = LegalizedScalarInstructionKind::EstablishReference {
        result: reference.clone(),
        source: StructuralArgument {
            place: backing,
            path: vec![],
            access: StructuralAccess::MutableBorrow,
        },
        shape: ValueShape::integer(8, 8),
    };
    source.blocks[0].instructions[2].kind =
        LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
            destination: StructuralScalarStoreDestination::OwnedResult {
                producer,
                result: reference,
            },
            path: vec![StructuralPathSegment::Referent],
            value: AbstractResult {
                value: ValueId::new(1).unwrap(),
                scalar_type: scalar,
            },
            byte_offset: 0,
            byte_size: 8,
            indices: vec![],
        };
    source.blocks[0].instructions[3].kind =
        LegalizedScalarInstructionKind::ReleaseReference { source: carrier };

    let constraints = SelectedSelectionConstraints {
        keys: environment.selected_keys(),
        fixed_inputs: vec![],
    };
    let selected = build(
        0,
        &source,
        target,
        &constraints,
        environment.physical(),
        environment.constraints(),
    )
    .expect("a directly established reference retains its original address");
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
    validate(&selected).expect("independent replay checks the dereferencing store");
    let loads = selected.blocks[0]
        .instructions
        .iter()
        .enumerate()
        .filter(|(_, instruction)| {
            matches!(instruction.kind, SelectedInstructionKind::Load64 { .. })
                && instruction.provenance.operations == [store]
        })
        .map(|(position, _)| position)
        .collect::<Vec<_>>();
    let [load] = loads.as_slice() else {
        panic!("the primitive store loads exactly one captured pointer");
    };
    let read = selected
        .memory_accesses
        .iter()
        .position(|access| {
            access.origin == SelectedMemoryAccessOrigin::Operation(store)
                && access.role == SelectedMemoryAccessRole::ReadPlace
        })
        .expect("the pointer load retains its read footprint");
    assert_eq!(selected.memory_accesses[read].place, carrier);
    assert_eq!(selected.memory_accesses[read].byte_offset, 0);
    assert_eq!(selected.memory_accesses[read].byte_count, 8);
    for mutation in 0..5 {
        let mut candidate = selected.clone();
        match mutation {
            0 => {
                candidate.blocks[0].instructions.remove(*load);
            }
            1 => candidate.blocks[0].instructions[*load].kind = SelectedInstructionKind::CopyI64,
            2 => {
                candidate.blocks[0].instructions[*load].kind =
                    SelectedInstructionKind::Load64 { byte_offset: 8 };
            }
            3 => {
                let value = candidate.blocks[0]
                    .instructions
                    .iter()
                    .find(|instruction| {
                        matches!(instruction.kind, SelectedInstructionKind::Store { .. })
                            && instruction.provenance.operations == [store]
                    })
                    .unwrap()
                    .operands[1]
                    .virtual_register;
                candidate.blocks[0].instructions[*load].operands[0].virtual_register = value;
            }
            _ => candidate.memory_accesses[read].place = backing,
        }
        assert!(
            validate(&candidate).is_err(),
            "pointer-load mutation {mutation}"
        );
    }
}

#[test]
fn borrowed_scalar_store_replay_rejects_changed_footprint_source_and_fuel() {
    use selected_instructions::SelectedMemoryAccessRole;
    use terminal_psi::{
        StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    };
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        for (bytes, scalar_type) in
            [(1_u8, ScalarType::Boolean)]
                .into_iter()
                .chain([1_u8, 2, 4, 8].into_iter().flat_map(|bytes| {
                    [IntegerSign::Signed, IntegerSign::Unsigned]
                        .into_iter()
                        .map(move |sign| {
                            (
                                bytes,
                                ScalarType::Integer(
                                    IntegerType::new(sign, u16::from(bytes) * 8).unwrap(),
                                ),
                            )
                        })
                }))
        {
            for (primitive, access) in [false, true].into_iter().flat_map(|primitive| {
                [
                    StructuralAccess::MutableBorrow,
                    StructuralAccess::WriteOnlyBorrow,
                ]
                .into_iter()
                .map(move |access| (primitive, access))
            }) {
                let environment =
                    register_environment::baseline_target_register_environment(target).unwrap();
                let mut source = fixture(target, 0);
                let structural_type = source.attachment.unwrap();
                if primitive {
                    source.attachment = None;
                }
                let place = semantic_vocabulary::PlaceId::new(1).unwrap();
                let field = semantic_vocabulary::StructuralFieldId::new(1).unwrap();
                source.call_plan = evaluate_call_plan(
                    CallingPolicy::native_for_target(target),
                    &CallSignature {
                        parameters: vec![ValueShape::borrowed_reference(
                            u32::from(bytes),
                            u16::from(bytes),
                        )],
                        result: None,
                    },
                )
                .unwrap();
                let destination = StructuralParameterDeclaration {
                    place,
                    position: 0,
                    is_self: !primitive,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                };
                source.structural = Some(legalized_operations::LegalizedStructuralContract {
                    result: None,
                    structural_types: vec![StructuralTypeDeclaration {
                        id: structural_type,
                        identity: "Record".into(),
                        shape: if primitive {
                            StructuralTypeShape::PrimitiveScalar(scalar_type)
                        } else {
                            StructuralTypeShape::Record {
                                fields: vec![StructuralFieldDeclaration {
                                    id: field,
                                    identity: "value".into(),
                                    relevance: terminal_psi::BindingRelevance::Relevant,
                                    field_type: StructuralFieldType::Scalar(scalar_type),
                                }],
                            }
                        },
                    }]
                    .into(),
                    parameters: vec![legalized_operations::LegalizedCallUnitParameter {
                        semantic: destination.clone(),
                        target: target_operations::TargetStructuralParameter {
                            place,
                            structural_type,
                            multiplicity: destination.multiplicity,
                            access,
                            projected_qualifications: Vec::new(),
                            shape: source.call_plan.parameters[0].shape,
                            placement: source.call_plan.parameters[0].clone(),
                        },
                    }],
                    structural_places: vec![terminal_psi::StructuralPlaceDeclaration {
                        id: place,
                        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                            position: 0,
                            is_self: !primitive,
                        },
                    }],
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                });
                source.blocks[0].instructions.truncate(2);
                source.provenance.operations.truncate(2);
                source.blocks[0].instructions[0]
                    .result
                    .as_mut()
                    .unwrap()
                    .scalar_type = scalar_type;
                source.blocks[0].instructions[1].result = None;
                source.blocks[0].instructions[1].kind = if primitive {
                    LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
                        path: Vec::new(),
                        indices: Vec::new(),
                        byte_offset: 0,
                        destination: destination.into(),
                        value: abstract_operations::AbstractResult {
                            value: ValueId::new(1).unwrap(),
                            scalar_type,
                        },
                        byte_size: bytes,
                    }
                } else {
                    LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                        destination: destination.into(),
                        path: Vec::new(),
                        indices: Vec::new(),
                        field,
                        value: abstract_operations::AbstractResult {
                            value: ValueId::new(1).unwrap(),
                            scalar_type,
                        },
                        byte_offset: 0,
                        byte_size: bytes,
                    }
                };
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
                if primitive {
                    for source_mutation in 0..4 {
                        let mut changed = source.clone();
                        let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
                            destination:
                                abstract_operations::StructuralScalarStoreDestination::Parameter(
                                    destination,
                                ),
                            value,
                            byte_size,
                            ..
                        } = &mut changed.blocks[0].instructions[1].kind
                        else {
                            panic!("primitive store");
                        };
                        match source_mutation {
                            0 => destination.access = StructuralAccess::SharedBorrow,
                            1 => *byte_size = if bytes == 8 { 4 } else { 8 },
                            2 => {
                                value.scalar_type = if scalar_type == ScalarType::Boolean {
                                    ScalarType::Integer(
                                        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                                    )
                                } else {
                                    ScalarType::Boolean
                                }
                            }
                            _ => {
                                changed
                                    .structural
                                    .as_mut()
                                    .unwrap()
                                    .structural_types
                                    .make_mut()[0]
                                    .shape = StructuralTypeShape::Record { fields: Vec::new() }
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
                            "source mutation {source_mutation}"
                        );
                        assert!(
                            crate::selection::validation::scalar_graph::validate(
                                0,
                                &changed,
                                &selected,
                                target,
                                &constraints,
                                environment.physical(),
                                environment.constraints()
                            )
                            .is_err(),
                            "replay source mutation {source_mutation}"
                        );
                    }
                }
                assert_eq!(selected.memory_accesses.len(), 1);
                assert_eq!(
                    selected.memory_accesses[0].role,
                    SelectedMemoryAccessRole::WritePlace
                );
                for mutation in 0..6 {
                    let mut candidate = selected.clone();
                    let instruction = candidate.blocks[0]
                        .instructions
                        .iter_mut()
                        .find(|instruction| {
                            matches!(instruction.kind, SelectedInstructionKind::Store { .. })
                        })
                        .unwrap();
                    match mutation {
                        0 => {
                            instruction.kind = SelectedInstructionKind::Store {
                                byte_offset: 1,
                                byte_size: bytes,
                            }
                        }
                        1 => {
                            instruction.kind = SelectedInstructionKind::Store {
                                byte_offset: 0,
                                byte_size: if bytes == 8 { 4 } else { 8 },
                            }
                        }
                        2 => instruction.provenance.fuel.clear(),
                        3 => {
                            candidate.memory_accesses[0].role = SelectedMemoryAccessRole::ReadPlace
                        }
                        4 => instruction.operands.swap(0, 1),
                        _ => {
                            let pointer = instruction.operands[0].virtual_register;
                            let value = instruction.operands[1].virtual_register;
                            let origin = candidate.virtual_registers[value.0 as usize].origin;
                            candidate.virtual_registers[pointer.0 as usize].origin = origin;
                        }
                    }
                    assert!(
                        validate(&candidate).is_err(),
                        "mutation {mutation}, width {bytes}, primitive {primitive}"
                    );
                }
            }
        }
    }
}
