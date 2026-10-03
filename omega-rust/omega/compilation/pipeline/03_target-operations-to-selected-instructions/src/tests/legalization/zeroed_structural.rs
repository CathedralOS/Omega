//! Zero initialization retains every field and array leaf through selection.
use crate::tests::fixtures::plain_unit::plain_unit_fixture;
use crate::{
    legalize_target_operations, select_instructions, selection_constraints,
    validate_selected_instructions,
};
use abstract_operations::{
    AbstractFunctionResult, AbstractOperation, AbstractResult, AbstractStructuralCallResult,
};
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
use semantic_vocabulary::{
    EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId, PlaceId,
    ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction,
};

#[test]
fn zeroed_mixed_arrays_keep_common_and_selected_case_cells_distinct() {
    use semantic_vocabulary::StructuralCaseId;
    use terminal_psi::StructuralCaseDeclaration;

    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        for has_default_payload in [false, true] {
            let (mut source, _, _) = plain_unit_fixture();
            let scalar = |width| {
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, width).unwrap())
            };
            let root = StructuralTypeId::new(1).unwrap();
            let array = StructuralTypeId::new(2).unwrap();
            let mixed = StructuralTypeId::new(3).unwrap();
            let nested = StructuralTypeId::new(4).unwrap();
            let field = |identity, name: &str, field_type| StructuralFieldDeclaration {
                id: StructuralFieldId::new(identity).unwrap(),
                identity: name.into(),
                relevance: BindingRelevance::Relevant,
                field_type,
            };
            source.structural_types = vec![
                StructuralTypeDeclaration {
                    id: root,
                    identity: "test::MixedArrayOwner".into(),
                    shape: StructuralTypeShape::Record {
                        fields: vec![
                            field(
                                1,
                                "prefix",
                                StructuralFieldType::Scalar(ScalarType::Boolean),
                            ),
                            field(2, "items", StructuralFieldType::Structural(array)),
                        ],
                    },
                },
                StructuralTypeDeclaration {
                    id: array,
                    identity: "test::MixedArray".into(),
                    shape: StructuralTypeShape::FixedArray {
                        element: mixed,
                        length: 2,
                    },
                },
                StructuralTypeDeclaration {
                    id: mixed,
                    identity: "test::Mixed".into(),
                    shape: StructuralTypeShape::Mixed {
                        fields: vec![
                            field(1, "common", StructuralFieldType::Scalar(scalar(32))),
                            field(2, "nested", StructuralFieldType::Structural(nested)),
                        ],
                        cases: vec![
                            StructuralCaseDeclaration {
                                id: StructuralCaseId::new(7).unwrap(),
                                identity: "Default".into(),
                                // Field ids are local to the common/payload
                                // namespace: these intentionally collide.
                                fields: if has_default_payload {
                                    vec![
                                        field(
                                            1,
                                            "payload",
                                            StructuralFieldType::Scalar(scalar(64)),
                                        ),
                                        field(
                                            2,
                                            "ready",
                                            StructuralFieldType::Scalar(ScalarType::Boolean),
                                        ),
                                    ]
                                } else {
                                    Vec::new()
                                },
                            },
                            StructuralCaseDeclaration {
                                id: StructuralCaseId::new(8).unwrap(),
                                identity: "Other".into(),
                                fields: vec![field(
                                    1,
                                    "unused",
                                    StructuralFieldType::Scalar(scalar(64)),
                                )],
                            },
                        ],
                    },
                },
                StructuralTypeDeclaration {
                    id: nested,
                    identity: "test::NestedCommon".into(),
                    shape: StructuralTypeShape::Record {
                        fields: vec![field(1, "value", StructuralFieldType::Scalar(scalar(16)))],
                    },
                },
            ]
            .into();
            let place = PlaceId::new(1).unwrap();
            source.functions[0].operations = vec![
                AbstractOperation::EstablishZeroedStructural {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: StructuralOperationResult {
                        place,
                        structural_type: root,
                        multiplicity: StructuralMultiplicity::Affine,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        qualification_establishments: Vec::new(),
                        claims: Vec::new(),
                    },
                },
                AbstractOperation::StructuralCaseMembership {
                    psi_operation: OperationId::new(2).unwrap(),
                    result: AbstractResult {
                        value: ValueId::new(1).unwrap(),
                        scalar_type: ScalarType::Boolean,
                    },
                    source: place,
                    path: vec![
                        terminal_psi::StructuralPathSegment::Field("items".into()),
                        terminal_psi::StructuralPathSegment::FixedIndex(1),
                    ],
                    case: StructuralCaseId::new(7).unwrap(),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(1).unwrap(),
                    cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place)],
                },
            ];
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = selection_constraints(&legal, &environment);
            let selected = select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .expect("mixed common storage and default-case payload select");
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .expect("mixed zero stores independently replay");
            let mut changed = selected.plan().clone();
            let tag_read = changed.functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find_map(|instruction| match &mut instruction.kind {
                    selected_instructions::SelectedInstructionKind::Load32 { byte_offset } => {
                        Some(byte_offset)
                    }
                    _ => None,
                })
                .expect("the zeroed local's projected tag remains readable");
            assert_eq!(*tag_read, if has_default_payload { 40 } else { 32 });
            *tag_read += 1;
            assert!(
                validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed,
                )
                .is_err(),
                "a projected tag cannot read neighboring storage"
            );
            let mut stores = selected.plan().functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| match instruction.kind {
                    selected_instructions::SelectedInstructionKind::Store {
                        byte_offset,
                        byte_size,
                    } => Some((byte_offset, byte_size)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            stores.sort();
            let expected = if has_default_payload {
                vec![
                    (0, 1),
                    (8, 4),
                    (12, 4),
                    (16, 2),
                    (24, 8),
                    (32, 1),
                    (40, 4),
                    (44, 4),
                    (48, 2),
                    (56, 8),
                    (64, 1),
                ]
            } else {
                vec![(0, 1), (8, 4), (12, 4), (16, 2), (32, 4), (36, 4), (40, 2)]
            };
            assert_eq!(stores, expected);
            for (offset, _) in stores {
                let mut hostile = selected.plan().clone();
                let instruction = hostile.functions[0]
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.instructions)
                    .find(|instruction| {
                        matches!(instruction.kind,
                        selected_instructions::SelectedInstructionKind::Store { byte_offset, .. }
                            if byte_offset == offset)
                    })
                    .unwrap();
                let selected_instructions::SelectedInstructionKind::Store { byte_offset, .. } =
                    &mut instruction.kind
                else {
                    unreachable!()
                };
                // Tags, common fields, nested common records and selected
                // payloads each owe their own exact byte range.
                *byte_offset += 1;
                assert!(
                    validate_selected_instructions(
                        &legal,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        hostile,
                    )
                    .is_err(),
                    "changed zero-store offset {offset} must not replay"
                );
            }
        }
    }
}

#[test]
fn zeroed_empty_record_borrowed_calls_replay_the_exact_address_anchor() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        for access in [
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
        ] {
            let (mut source, _, _) = plain_unit_fixture();
            let structural_type = StructuralTypeId::new(1).unwrap();
            let place = PlaceId::new(1).unwrap();
            let producer = OperationId::new(1).unwrap();
            source.structural_types = vec![StructuralTypeDeclaration {
                id: structural_type,
                identity: "test::Empty".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            }]
            .into();
            let mut callee = source.functions[0].clone();
            callee.machine = MachineId::new(2).unwrap();
            callee.structural_parameters = vec![StructuralParameterDeclaration {
                place: PlaceId::new(2).unwrap(),
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }];
            source.functions[0].operations = vec![
                AbstractOperation::EstablishZeroedStructural {
                    psi_operation: producer,
                    result: StructuralOperationResult {
                        place,
                        structural_type,
                        multiplicity: StructuralMultiplicity::Affine,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        qualification_establishments: Vec::new(),
                        claims: Vec::new(),
                    },
                },
                AbstractOperation::CallStructural {
                    psi_operation: OperationId::new(2).unwrap(),
                    callee: callee.machine,
                    arguments: Vec::new(),
                    structural_arguments: vec![StructuralArgument {
                        place,
                        access,
                        path: Vec::new(),
                    }],
                    dynamic_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                    result: AbstractStructuralCallResult::Unit,
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(1).unwrap(),
                    cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place)],
                },
            ];
            source.functions.push(callee);
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            optimization_unit_semantics::validate_psi_optimization_unit(&unit)
                .expect("borrowing the zeroed empty record retains its owner's cleanup");
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
            let target_operations::TargetUnitOperation::Call { arguments, .. } =
                &target.functions[0].graph.blocks[0].operations[1]
            else {
                unreachable!()
            };
            assert_eq!(
                arguments[0].shape,
                calling_conventions::ValueShape::borrowed_reference(0, 1)
            );
            assert_eq!(arguments[0].access, access);
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = selection_constraints(&legal, &environment);
            let selected = select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .expect("a zeroed empty record retains a borrowable address");
            assert_eq!(
                selected.plan().functions[0].local_storage_slots,
                vec![SelectedLocalStorageSlot {
                    id: LocalStorageSlotId::Structural {
                        operation: producer,
                        place
                    },
                    byte_size: 1,
                    alignment: 1,
                }]
            );
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .expect("independent replay reconstructs the same anchor and borrowed pointer");
            for mutation in 0..4 {
                let mut hostile = selected.plan().clone();
                let slots = &mut hostile.functions[0].local_storage_slots;
                match mutation {
                    0 => slots.clear(),
                    1 => slots[0].byte_size = 0,
                    2 => {
                        slots[0].id = LocalStorageSlotId::Structural {
                            operation: producer,
                            place: PlaceId::new(99).unwrap(),
                        }
                    }
                    _ => {
                        slots[0].id = LocalStorageSlotId::Structural {
                            operation: OperationId::new(99).unwrap(),
                            place,
                        }
                    }
                }
                assert!(
                    validate_selected_instructions(
                        &legal,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        hostile,
                    )
                    .is_err(),
                    "missing, resized, or foreign-owner anchors must not replay"
                );
            }
        }
    }
}

#[test]
fn zeroed_records_and_array_leaves_select_exact_stores_and_reads() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        for multiplicity in [
            StructuralMultiplicity::Unrestricted,
            StructuralMultiplicity::Affine,
        ] {
            let (mut source, _, _) = plain_unit_fixture();
            let scalar = |width| {
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, width).unwrap())
            };
            let root = StructuralTypeId::new(1).unwrap();
            let array = StructuralTypeId::new(2).unwrap();
            let element = StructuralTypeId::new(3).unwrap();
            let field = |identity, name: &str, field_type| StructuralFieldDeclaration {
                id: StructuralFieldId::new(identity).unwrap(),
                identity: name.into(),
                relevance: BindingRelevance::Relevant,
                field_type,
            };
            source.structural_types = vec![
                StructuralTypeDeclaration {
                    id: root,
                    identity: "test::Zeroed".into(),
                    shape: StructuralTypeShape::Record {
                        fields: vec![
                            field(1, "tag", StructuralFieldType::Scalar(ScalarType::Boolean)),
                            field(2, "wide", StructuralFieldType::Scalar(scalar(64))),
                            field(3, "items", StructuralFieldType::Structural(array)),
                        ],
                    },
                },
                StructuralTypeDeclaration {
                    id: array,
                    identity: "test::Array".into(),
                    shape: StructuralTypeShape::FixedArray { element, length: 3 },
                },
                StructuralTypeDeclaration {
                    id: element,
                    identity: "test::Element".into(),
                    shape: StructuralTypeShape::PrimitiveScalar(scalar(16)),
                },
            ]
            .into();
            let place = PlaceId::new(1).unwrap();
            source.functions[0].operations.insert(
                0,
                AbstractOperation::EstablishZeroedStructural {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: StructuralOperationResult {
                        place,
                        structural_type: root,
                        multiplicity,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        qualification_establishments: Vec::new(),
                        claims: Vec::new(),
                    },
                },
            );
            let read = ValueId::new(1).unwrap();
            let returned = ValueId::new(2).unwrap();
            let AbstractOperation::ReturnUnit { psi_edge, .. } = source.functions[0].operations[1]
            else {
                panic!("unit return");
            };
            source.functions[0].result = AbstractFunctionResult::Scalar(AbstractResult {
                value: returned,
                scalar_type: scalar(64),
            });
            source.functions[0].operations[1] = AbstractOperation::StructuralScalarFieldRead {
                psi_operation: OperationId::new(2).unwrap(),
                result: AbstractResult {
                    value: read,
                    scalar_type: scalar(64),
                },
                source: place,
                path: Vec::new(),
                field: StructuralFieldId::new(2).unwrap(),
            };
            source.functions[0]
                .operations
                .push(AbstractOperation::Return {
                    psi_edge,
                    result: returned,
                    value: read,
                    scalar_type: scalar(64),
                    cleanup_actions: if multiplicity == StructuralMultiplicity::Affine {
                        vec![TerminalAffineCleanupAction::DiscardRoot(place)]
                    } else {
                        Vec::new()
                    },
                });
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = selection_constraints(&legal, &environment);
            let selected = select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .expect("zeroed structural storage selects");
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();
            let mut stores = selected.plan().functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| {
                    if let selected_instructions::SelectedInstructionKind::Store {
                        byte_offset,
                        byte_size,
                    } = instruction.kind
                    {
                        Some((byte_offset, byte_size))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            stores.sort();
            assert_eq!(stores, [(0, 1), (8, 8), (16, 2), (18, 2), (20, 2)]);
            assert!(
                selected.plan().functions[0]
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .any(|instruction| matches!(
                        instruction.kind,
                        selected_instructions::SelectedInstructionKind::Load64 { byte_offset: 8 }
                    ))
            );
            let mut changed_read = selected.plan().clone();
            let read = changed_read.functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|instruction| {
                    matches!(
                        instruction.kind,
                        selected_instructions::SelectedInstructionKind::Load64 { .. }
                    )
                })
                .unwrap();
            let selected_instructions::SelectedInstructionKind::Load64 { byte_offset } =
                &mut read.kind
            else {
                unreachable!()
            };
            *byte_offset = 0;
            assert!(
                validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed_read
                )
                .is_err()
            );
            let mut changed = selected.plan().clone();
            let store = changed.functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|instruction| {
                    matches!(
                        instruction.kind,
                        selected_instructions::SelectedInstructionKind::Store {
                            byte_offset: 8,
                            ..
                        }
                    )
                })
                .unwrap();
            let selected_instructions::SelectedInstructionKind::Store { byte_offset, .. } =
                &mut store.kind
            else {
                unreachable!()
            };
            *byte_offset = 0;
            assert!(
                validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed
                )
                .is_err()
            );
        }
    }
}
