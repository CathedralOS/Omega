use super::*;
use StructuralPathSegment::{Field, FixedIndex};
use assigned_target_operations::AssignedAggregateCopy;
use calling_conventions::ValueShape;
use semantic_vocabulary::{EdgeId, OperationId, StructuralFieldId};
use target_operations::{TargetStructuralParameter, TerminalPsiProvenance};
use terminal_psi::{
    BindingRelevance, StructuralAffineDiscard, StructuralFieldDeclaration, StructuralFieldType,
    StructuralTypeDeclaration, StructuralTypeShape,
};

fn identity(value: u32) -> StructuralTypeId {
    StructuralTypeId::new(u64::from(value)).unwrap()
}

fn declaration(value: u32, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: identity(value),
        identity: format!("Type{value}"),
        shape,
    }
}

fn token() -> StructuralTypeDeclaration {
    declaration(
        3,
        StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(1).unwrap(),
                identity: "value".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .unwrap(),
                )),
            }],
        },
    )
}

fn fixture(
    target: NativeTarget,
    declarations: Vec<StructuralTypeDeclaration>,
    root_shape: ValueShape,
    metadata: (Option<u64>, Option<u32>),
    moves: Vec<(
        Vec<StructuralPathSegment>,
        StructuralTypeId,
        ValueShape,
        u32,
    )>,
    residuals: Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> (AssignedUnitBody, Vec<AssignedFunction>) {
    let root_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![root_shape],
            result: None,
        },
    )
    .unwrap();
    let place = PlaceId::new(1).unwrap();
    let mut body = AssignedUnitBody {
        structural_types: declarations.clone(),
        call_plan: root_plan.clone(),
        scalar_parameters: Vec::new(),
        parameters: vec![TargetStructuralParameter {
            place,
            structural_type: identity(1),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            projected_qualifications: Vec::new(),
            shape: root_shape,
            placement: root_plan.parameters[0].clone(),
        }],
        operations: Vec::new(),
    };
    let mut functions = Vec::new();
    for (ordinal, (path, structural_type, shape, offset)) in moves.into_iter().enumerate() {
        let machine = MachineId::new(u64::try_from(ordinal).unwrap() + 2).unwrap();
        let operation = OperationId::new(u64::try_from(ordinal).unwrap() + 1).unwrap();
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![shape],
                result: None,
            },
        )
        .unwrap();
        let callee_place = PlaceId::new(u64::try_from(ordinal).unwrap() + 2).unwrap();
        body.operations.push(AssignedUnitOperation::Call {
            transport: None,
            psi_operation: operation,
            callee: machine,
            result: None,
            call_plan: call_plan.clone(),
            scalar_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            copies: vec![AssignedAggregateCopy {
                place,
                access: StructuralAccess::Owned,
                path,
                root_structural_type: identity(1),
                structural_type,
                shape,
                source_byte_offset: offset,
                fixed_array_length: metadata.0,
                element_stride: metadata.1,
                source: root_plan.parameters[0].clone(),
                destination: call_plan.parameters[0].clone(),
            }],
        });
        functions.push(AssignedFunction {
            machine,
            attachment: None,
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: Vec::new(),
                edges: vec![EdgeId::new(1).unwrap()],
            },
            operation: AssignedOperation::UnitBody(AssignedUnitBody {
                structural_types: declarations.clone(),
                call_plan: call_plan.clone(),
                scalar_parameters: Vec::new(),
                parameters: vec![TargetStructuralParameter {
                    place: callee_place,
                    structural_type,
                    shape,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    projected_qualifications: Vec::new(),
                    placement: call_plan.parameters[0].clone(),
                }],
                operations: vec![AssignedUnitOperation::Return {
                    psi_edge: EdgeId::new(1).unwrap(),
                    cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(callee_place)],
                }],
            }),
        });
    }
    body.operations.push(AssignedUnitOperation::Return {
        psi_edge: EdgeId::new(1).unwrap(),
        cleanup_actions: residuals
            .into_iter()
            .map(|(path, structural_type)| {
                TerminalAffineCleanupAction::DiscardResidual(StructuralAffineDiscard {
                    place,
                    path,
                    structural_type,
                })
            })
            .collect(),
    });
    (body, functions)
}

fn five(target: NativeTarget) -> (AssignedUnitBody, Vec<AssignedFunction>) {
    use StructuralPathSegment::FixedIndex;
    fixture(
        target,
        vec![
            declaration(
                1,
                StructuralTypeShape::FixedArray {
                    element: identity(3),
                    length: 5,
                },
            ),
            token(),
        ],
        ValueShape::integer(40, 8),
        (Some(5), Some(8)),
        vec![(
            vec![FixedIndex(3)],
            identity(3),
            ValueShape::integer(8, 8),
            24,
        )],
        [4, 2, 1, 0]
            .into_iter()
            .map(|index| (vec![FixedIndex(index)], identity(3)))
            .collect(),
    )
}

fn accepts(body: &AssignedUnitBody, functions: &[AssignedFunction], target: NativeTarget) -> bool {
    validate_projected_cleanup(
        body,
        Some(MachineId::new(1).unwrap()),
        None,
        target,
        functions,
    )
    .is_ok()
}

fn structural_field(value: u32, name: &str, nested: u32) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(u64::from(value)).unwrap(),
        identity: name.into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(identity(nested)),
    }
}

#[test]
fn bounded_byte_fields_preserve_layout_and_authored_call_ordinals() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let fields = vec![
            StructuralFieldDeclaration {
                field_type: StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Boolean),
                ..structural_field(1, "flag", 3)
            },
            StructuralFieldDeclaration {
                field_type: StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 5 },
                ),
                ..structural_field(2, "bytes", 3)
            },
            StructuralFieldDeclaration {
                field_type: StructuralFieldType::IeeeFloat(
                    semantic_vocabulary::IeeeFloatFormat::Binary32,
                ),
                ..structural_field(3, "ratio", 3)
            },
            structural_field(4, "first", 3),
            structural_field(5, "second", 3),
        ];
        // flag at 0; the 13-byte bounded carrier at 8; ratio at 24;
        // structural fields at 32 and 40. Scalar storage has no cleanup.
        let (body, functions) = fixture(
            target,
            vec![
                declaration(1, StructuralTypeShape::Record { fields }),
                token(),
            ],
            ValueShape::integer(48, 8),
            (None, None),
            vec![
                (
                    vec![Field("second".into())],
                    identity(3),
                    ValueShape::integer(8, 8),
                    40,
                ),
                (
                    vec![Field("first".into())],
                    identity(3),
                    ValueShape::integer(8, 8),
                    32,
                ),
            ],
            Vec::new(),
        );
        let emitted = super::super::emit_unit_body(
            &body,
            Some(MachineId::new(1).unwrap()),
            None,
            target,
            &functions,
            &[],
        )
        .unwrap();
        assert!(emitted.affine_cleanup.as_ref().unwrap().actions.is_empty());
        assert_eq!(emitted.internal_unit_calls.len(), 2);
        for (ordinal, call) in emitted.internal_unit_calls.iter().enumerate() {
            assert_eq!(call.operation_ordinal, ordinal);
            assert_eq!(
                call.owner,
                target_operations::CallSiteOwner::Operation(
                    OperationId::new(u64::try_from(ordinal).unwrap() + 1).unwrap(),
                )
            );
            assert_eq!(call.arguments[0].source_byte_offset, [40, 32][ordinal]);
            assert_eq!(call.arguments[0].fixed_array_length, None);
            assert_eq!(call.arguments[0].element_stride, None);
        }
        let mut duplicated = body.clone();
        let AssignedUnitOperation::Call { psi_operation, .. } = &mut duplicated.operations[1]
        else {
            unreachable!()
        };
        *psi_operation = OperationId::new(1).unwrap();
        assert!(!accepts(&duplicated, &functions, target));

        let mut wrong_offset = body.clone();
        let AssignedUnitOperation::Call { copies, .. } = &mut wrong_offset.operations[0] else {
            unreachable!()
        };
        copies[0].source_byte_offset = 32;
        assert!(!accepts(&wrong_offset, &functions, target));

        let mut overflow = body.clone();
        let StructuralTypeShape::Record { fields } = &mut overflow.structural_types[0].shape else {
            unreachable!()
        };
        fields[1].field_type =
            StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                capacity: u64::MAX,
            });
        assert!(!accepts(&overflow, &functions, target));
    }
}

#[test]
fn mixed_record_root_paths_derive_offset_without_array_metadata() {
    let target = NativeTarget::linux_x64();
    let (body, functions) = fixture(
        target,
        vec![
            declaration(
                1,
                StructuralTypeShape::Record {
                    fields: vec![
                        structural_field(1, "prefix", 3),
                        structural_field(2, "rows", 2),
                    ],
                },
            ),
            declaration(
                2,
                StructuralTypeShape::FixedArray {
                    element: identity(4),
                    length: 3,
                },
            ),
            token(),
            declaration(
                4,
                StructuralTypeShape::Record {
                    fields: vec![
                        structural_field(3, "head", 3),
                        structural_field(4, "tail", 5),
                    ],
                },
            ),
            declaration(
                5,
                StructuralTypeShape::FixedArray {
                    element: identity(3),
                    length: 3,
                },
            ),
        ],
        ValueShape::integer(104, 8),
        (None, None),
        vec![(
            vec![
                Field("rows".into()),
                FixedIndex(1),
                Field("tail".into()),
                FixedIndex(1),
            ],
            identity(3),
            ValueShape::integer(8, 8),
            56,
        )],
        vec![
            (vec![Field("rows".into()), FixedIndex(2)], identity(4)),
            (
                vec![
                    Field("rows".into()),
                    FixedIndex(1),
                    Field("tail".into()),
                    FixedIndex(2),
                ],
                identity(3),
            ),
            (
                vec![
                    Field("rows".into()),
                    FixedIndex(1),
                    Field("tail".into()),
                    FixedIndex(0),
                ],
                identity(3),
            ),
            (
                vec![Field("rows".into()), FixedIndex(1), Field("head".into())],
                identity(3),
            ),
            (vec![Field("rows".into()), FixedIndex(0)], identity(4)),
            (vec![Field("prefix".into())], identity(3)),
        ],
    );
    assert!(accepts(&body, &functions, target));
    let mut forged = body.clone();
    let AssignedUnitOperation::Call { copies, .. } = &mut forged.operations[0] else {
        unreachable!()
    };
    copies[0].fixed_array_length = Some(3);
    copies[0].element_stride = Some(8);
    assert!(!accepts(&forged, &functions, target));
}

#[test]
fn whole_array_rows_emit_indirect_temporary_copies() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (body, functions) = fixture(
            target,
            vec![
                declaration(
                    1,
                    StructuralTypeShape::FixedArray {
                        element: identity(2),
                        length: 3,
                    },
                ),
                declaration(
                    2,
                    StructuralTypeShape::FixedArray {
                        element: identity(3),
                        length: 3,
                    },
                ),
                token(),
            ],
            ValueShape::integer(72, 8),
            (Some(3), Some(24)),
            vec![(
                vec![FixedIndex(1)],
                identity(2),
                ValueShape::integer(24, 8),
                24,
            )],
            vec![
                (vec![FixedIndex(2)], identity(2)),
                (vec![FixedIndex(0)], identity(2)),
            ],
        );
        let emitted = super::super::emit_unit_body(
            &body,
            Some(MachineId::new(1).unwrap()),
            None,
            target,
            &functions,
            &[],
        )
        .unwrap();
        let argument = &emitted.internal_unit_calls[0].arguments[0];
        assert_eq!(argument.source_byte_offset, 24);
        assert_eq!(argument.shape, ValueShape::integer(24, 8));
        assert!(!argument.bytes.is_empty());
        if let [
            calling_conventions::ValueLocation::Indirect {
                copy_stack_byte_offset: Some(offset),
                byte_size,
                ..
            },
        ] = argument.destination.locations.as_slice()
        {
            assert!(
                offset + u32::from(*byte_size) <= argument.call_stack_bytes,
                "the whole subtree copy stays below the caller's saved homes"
            );
        }
    }
}

#[test]
fn empty_attachments_are_accepted_and_nonempty_attachments_rejected() {
    let target = NativeTarget::linux_x64();
    let (mut body, mut functions) = five(target);
    let attachment = declaration(4, StructuralTypeShape::Record { fields: Vec::new() });
    body.structural_types.push(attachment.clone());
    let AssignedOperation::UnitBody(callee) = &mut functions[0].operation else {
        unreachable!()
    };
    callee.structural_types.push(attachment);
    functions[0].attachment = Some(identity(4));
    assert!(
        validate_projected_cleanup(
            &body,
            Some(MachineId::new(1).unwrap()),
            Some(identity(4)),
            target,
            &functions
        )
        .is_ok()
    );
    assert!(
        validate_projected_cleanup(
            &body,
            Some(MachineId::new(1).unwrap()),
            Some(identity(3)),
            target,
            &functions
        )
        .is_err()
    );
    functions[0].attachment = Some(identity(3));
    assert!(!accepts(&body, &functions, target));
}

#[test]
fn linear_projection_and_write_only_borrow_keep_their_existing_routes() {
    let target = NativeTarget::linux_x64();
    for borrowed in [false, true] {
        let (mut body, functions) = five(target);
        let AssignedUnitOperation::Return {
            cleanup_actions, ..
        } = body.operations.last_mut().unwrap()
        else {
            unreachable!()
        };
        cleanup_actions.clear();
        if borrowed {
            body.parameters[0].access = StructuralAccess::WriteOnlyBorrow;
            let AssignedUnitOperation::Call { copies, .. } = &mut body.operations[0] else {
                unreachable!()
            };
            copies[0].access = StructuralAccess::WriteOnlyBorrow;
            copies[0].fixed_array_length = None;
            copies[0].element_stride = None;
        } else {
            body.parameters[0].multiplicity = StructuralMultiplicity::Linear;
        }
        assert_eq!(
            validate_projected_cleanup(
                &body,
                Some(MachineId::new(1).unwrap()),
                None,
                target,
                &functions
            )
            .unwrap(),
            None
        );
    }
}

#[test]
fn five_elements_emit_calls_and_no_code_residual_cleanup_on_each_abi() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let (body, functions) = five(target);
        assert!(accepts(&body, &functions, target));
        let emitted = super::super::emit_unit_body(
            &body,
            Some(MachineId::new(1).unwrap()),
            None,
            target,
            &functions,
            &[],
        )
        .unwrap();
        assert_eq!(emitted.internal_unit_calls.len(), 1);
        assert_eq!(
            emitted.internal_unit_calls[0].arguments[0].source_byte_offset,
            24
        );
        let cleanup = emitted.affine_cleanup.unwrap();
        let AssignedUnitOperation::Return {
            cleanup_actions, ..
        } = body.operations.last().unwrap()
        else {
            unreachable!()
        };
        assert_eq!(&cleanup.actions, cleanup_actions);
    }
}

#[test]
fn projected_replay_rejects_each_copy_join_and_missing_residual() {
    let target = NativeTarget::linux_x64();
    let (body, functions) = five(target);
    for mutation in 0..12 {
        let mut invalid = body.clone();
        let AssignedUnitOperation::Call {
            copies,
            call_plan,
            callee,
            ..
        } = &mut invalid.operations[0]
        else {
            unreachable!()
        };
        let copy = &mut copies[0];
        match mutation {
            0 => copy.source_byte_offset += 8,
            1 => copy.fixed_array_length = Some(4),
            2 => copy.element_stride = Some(16),
            3 => copy.structural_type = identity(1),
            4 => copy.root_structural_type = identity(3),
            5 => copy.place = PlaceId::new(99).unwrap(),
            6 => copy.access = StructuralAccess::WriteOnlyBorrow,
            7 => copy.source.shape.byte_size += 8,
            8 => copy.shape.alignment = 4,
            9 => copy.destination.locations.clear(),
            10 => call_plan.shadow_bytes += 8,
            11 => *callee = MachineId::new(99).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            !accepts(&invalid, &functions, target),
            "mutation {mutation}"
        );
    }
    let mut invalid = body.clone();
    let AssignedUnitOperation::Return {
        cleanup_actions, ..
    } = invalid.operations.last_mut().unwrap()
    else {
        unreachable!()
    };
    cleanup_actions.pop();
    assert!(!accepts(&invalid, &functions, target));
    let mut invalid_functions = functions.clone();
    let AssignedOperation::UnitBody(callee) = &mut invalid_functions[0].operation else {
        unreachable!()
    };
    callee.parameters[0].structural_type = identity(1);
    assert!(!accepts(&body, &invalid_functions, target));
}

#[test]
fn nested_paths_use_root_stride_and_keep_whole_untouched_rows() {
    use StructuralPathSegment::FixedIndex;
    let target = NativeTarget::linux_x64();
    let (body, functions) = fixture(
        target,
        vec![
            declaration(
                1,
                StructuralTypeShape::FixedArray {
                    element: identity(2),
                    length: 3,
                },
            ),
            declaration(
                2,
                StructuralTypeShape::FixedArray {
                    element: identity(3),
                    length: 3,
                },
            ),
            token(),
        ],
        ValueShape::integer(72, 8),
        (Some(3), Some(24)),
        vec![
            (
                vec![FixedIndex(1), FixedIndex(2)],
                identity(3),
                ValueShape::integer(8, 8),
                40,
            ),
            (
                vec![FixedIndex(1), FixedIndex(0)],
                identity(3),
                ValueShape::integer(8, 8),
                24,
            ),
        ],
        vec![
            (vec![FixedIndex(2)], identity(2)),
            (vec![FixedIndex(1), FixedIndex(1)], identity(3)),
            (vec![FixedIndex(0)], identity(2)),
        ],
    );
    assert!(accepts(&body, &functions, target));
    let mut wrong_metadata = body.clone();
    let AssignedUnitOperation::Call { copies, .. } = &mut wrong_metadata.operations[0] else {
        unreachable!()
    };
    copies[0].element_stride = Some(8);
    assert!(!accepts(&wrong_metadata, &functions, target));
}

#[test]
fn full_array_complement_is_empty_only_after_every_element_moves() {
    use StructuralPathSegment::FixedIndex;
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let (body, functions) = fixture(
            target,
            vec![
                declaration(
                    1,
                    StructuralTypeShape::FixedArray {
                        element: identity(3),
                        length: 3,
                    },
                ),
                token(),
            ],
            ValueShape::integer(24, 8),
            (Some(3), Some(8)),
            [2, 0, 1]
                .into_iter()
                .map(|index| {
                    (
                        vec![FixedIndex(index)],
                        identity(3),
                        ValueShape::integer(8, 8),
                        index as u32 * 8,
                    )
                })
                .collect(),
            Vec::new(),
        );
        assert!(accepts(&body, &functions, target));
        assert!(
            super::super::emit_unit_body(
                &body,
                Some(MachineId::new(1).unwrap()),
                None,
                target,
                &functions,
                &[]
            )
            .is_ok()
        );
        let mut missing = body.clone();
        missing.operations.remove(1);
        assert!(!accepts(&missing, &functions, target));
    }
}
