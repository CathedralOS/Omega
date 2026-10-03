//! Exact array argument placements and borrowed array presentation contracts.
use super::{
    CallSignature, CallingPolicy, IntegerSign, LegalizedScalarArgument, LegalizedScalarCall,
    LegalizedScalarFunction, ScalarType, StructuralAccess, ValueLocation, ValueShape,
    evaluate_call_plan,
};
use crate::selection::scalar_call_abi::validate_borrowed_argument;
use legalized_operations::{LegalizedCallUnitParameter, LegalizedStructuralContract};
use semantic_vocabulary::{
    BlockId, IntegerType, MachineId, OperationId, PlaceId, StructuralTypeId,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
};

fn fixed_array_call(
    target: target::NativeTarget,
) -> (LegalizedScalarFunction, LegalizedScalarCall) {
    let place = PlaceId::new(1).unwrap();
    let array_type = StructuralTypeId::new(1).unwrap();
    let byte_type = StructuralTypeId::new(2).unwrap();
    let view_type = StructuralTypeId::new(3).unwrap();
    let root_shape = ValueShape::borrowed_reference(17, 1);
    let view_shape = ValueShape::borrowed_reference(16, 8);
    let policy = CallingPolicy::native_for_target(target);
    let incoming = evaluate_call_plan(
        policy,
        &CallSignature {
            parameters: vec![root_shape],
            result: None,
        },
    )
    .unwrap();
    let outgoing = evaluate_call_plan(
        policy,
        &CallSignature {
            parameters: vec![view_shape],
            result: None,
        },
    )
    .unwrap();
    let placement = incoming.parameters[0].clone();
    let source = LegalizedScalarFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        provenance: target_operations::TerminalPsiProvenance {
            operations: Vec::new(),
            edges: Vec::new(),
        },
        call_plan: incoming,
        parameters: Vec::new(),
        dynamic_parameters: Vec::new(),
        entry_block: BlockId::new(1).unwrap(),
        // This test supplies only the signature consumed by the argument reader;
        // it makes no claim of graph or source admission.
        blocks: Vec::new(),
        structural: Some(LegalizedStructuralContract {
            result: None,
            structural_types: vec![
                StructuralTypeDeclaration {
                    id: array_type,
                    identity: "array".into(),
                    shape: StructuralTypeShape::FixedArray {
                        element: byte_type,
                        length: 17,
                    },
                },
                StructuralTypeDeclaration {
                    id: byte_type,
                    identity: "byte".into(),
                    shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    )),
                },
                StructuralTypeDeclaration {
                    id: view_type,
                    identity: "view".into(),
                    shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                    }),
                },
            ]
            .into(),
            parameters: vec![LegalizedCallUnitParameter {
                semantic: StructuralParameterDeclaration {
                    place,
                    position: 0,
                    is_self: false,
                    structural_type: array_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::MutableBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
                target: target_operations::TargetStructuralParameter {
                    place,
                    structural_type: array_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::MutableBorrow,
                    projected_qualifications: Vec::new(),
                    shape: root_shape,
                    placement: placement.clone(),
                },
            }],
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
        }),
        ownership_frontier_facts: Vec::new(),
    };
    let call = LegalizedScalarCall {
        descriptor_origins: Vec::new(),
        structural_result: None,
        source: legalized_operations::NativeCallOrigin::Authored,
        callee: MachineId::new(2).unwrap(),
        arguments: vec![LegalizedScalarArgument::Structural {
            indices: Vec::new(),
            semantic: StructuralArgument {
                place,
                access: StructuralAccess::MutableBorrow,
                path: Vec::new(),
            },
            target: target_operations::TargetStructuralArgument {
                place,
                access: StructuralAccess::MutableBorrow,
                path: Vec::new(),
                root_structural_type: array_type,
                structural_type: view_type,
                shape: view_shape,
                source_byte_offset: 0,
                fixed_array_length: Some(17),
                element_stride: Some(1),
                indices: Vec::new(),
                source: placement.into(),
                destination: outgoing.parameters[0].clone(),
            },
        }],
        call_plan: outgoing,
        result_placement: None,
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    (source, call)
}

#[test]
fn fixed_byte_call_windows_replay_offset_extent_and_access() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        for (start, end) in [(0, 0), (17, 17), (1, 3), (0, 17)] {
            let (source, mut call) = fixed_array_call(native);
            let LegalizedScalarArgument::Structural {
                semantic, target, ..
            } = &mut call.arguments[0]
            else {
                unreachable!()
            };
            semantic
                .path
                .push(terminal_psi::StructuralPathSegment::FixedByteRange { start, end });
            target.path = semantic.path.clone();
            target.source_byte_offset = start as u32;
            target.fixed_array_length = Some(end - start);
            let operation = OperationId::new(1).unwrap();
            assert_eq!(
                validate_borrowed_argument(&source, &call, operation, 0),
                Some(())
            );
            for mutation in 0..6 {
                let mut changed = call.clone();
                let LegalizedScalarArgument::Structural {
                    semantic, target, ..
                } = &mut changed.arguments[0]
                else {
                    unreachable!()
                };
                match mutation {
                    0 => target.source_byte_offset += 1,
                    1 => target.fixed_array_length = Some(end - start + 1),
                    2 => target.element_stride = Some(2),
                    3 => {
                        semantic.path[0] = terminal_psi::StructuralPathSegment::FixedByteRange {
                            start: 0,
                            end: 18,
                        };
                        target.path = semantic.path.clone();
                    }
                    4 => {
                        semantic.access = StructuralAccess::WriteOnlyBorrow;
                        target.access = semantic.access;
                    }
                    _ => target.path.clear(),
                }
                assert_eq!(
                    validate_borrowed_argument(&source, &changed, operation, 0),
                    None,
                    "mutation {mutation}"
                );
            }
        }
    }
}

#[test]
fn bounded_byte_calls_replay_the_declared_extent_and_pointer_cell() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let (mut source, mut call) = fixed_array_call(native);
        let signature = source.structural.as_mut().unwrap();
        let mut declarations = signature.structural_types.to_vec();
        declarations[2].shape =
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                access: Some(StructuralAccess::MutableBorrow),
                capacity: 17,
            });
        signature.structural_types = declarations.into();
        let shape = ValueShape::borrowed_reference(8, 8);
        call.call_plan = evaluate_call_plan(
            call.call_plan.policy,
            &CallSignature {
                parameters: vec![shape],
                result: None,
            },
        )
        .unwrap();
        let LegalizedScalarArgument::Structural { target, .. } = &mut call.arguments[0] else {
            panic!("fixed-array loan argument");
        };
        target.shape = shape;
        target.destination = call.call_plan.parameters[0].clone();
        let operation = OperationId::new(1).unwrap();
        assert_eq!(
            validate_borrowed_argument(&source, &call, operation, 0),
            Some(())
        );

        let mut wrong_extent = call.clone();
        let LegalizedScalarArgument::Structural { target, .. } = &mut wrong_extent.arguments[0]
        else {
            panic!("fixed-array loan argument");
        };
        target.fixed_array_length = Some(16);
        assert_eq!(
            validate_borrowed_argument(&source, &wrong_extent, operation, 0),
            None
        );

        // A self-consistent two-word ABI cannot replace the declared pointer
        // cell: replay derives geometry from the type, not the retained rows.
        let mut wrong_layout = call.clone();
        let wide_shape = ValueShape::borrowed_reference(16, 8);
        wrong_layout.call_plan = evaluate_call_plan(
            call.call_plan.policy,
            &CallSignature {
                parameters: vec![wide_shape],
                result: None,
            },
        )
        .unwrap();
        let LegalizedScalarArgument::Structural { target, .. } = &mut wrong_layout.arguments[0]
        else {
            panic!("fixed-array loan argument");
        };
        target.shape = wide_shape;
        target.destination = wrong_layout.call_plan.parameters[0].clone();
        assert_eq!(
            validate_borrowed_argument(&source, &wrong_layout, operation, 0),
            None
        );
    }
}

#[test]
fn owned_record_array_geometry_retains_padding_and_rejects_invalid_contents() {
    let (source, _) = fixed_array_call(target::NativeTarget::linux_x64());
    // Geometry alone: the unrelated borrowed call fixture is not re-admitted.
    let mut types = source.structural.unwrap().structural_types.to_vec();
    let array = types[0].id;
    let record = types[1].id;
    let borrowed_view = types[2].id;
    types[0].shape = StructuralTypeShape::FixedArray {
        element: record,
        length: 3,
    };
    types[1].shape = StructuralTypeShape::Record {
        fields: [(1, 8), (2, 64)]
            .into_iter()
            .map(|(ordinal, bits)| terminal_psi::StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(ordinal).unwrap(),
                identity: format!("field{ordinal}"),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
                )),
            })
            .collect(),
    };
    let shape = crate::structural_inputs::structural_reference_input::owned_aggregate_shape;
    assert_eq!(shape(record, &types), Some(ValueShape::integer(16, 8)));
    assert_eq!(shape(array, &types), Some(ValueShape::integer(48, 8)));
    for mutation in 0..6 {
        let mut changed = types.clone();
        match mutation {
            0 => {
                changed[0].shape = StructuralTypeShape::FixedArray {
                    element: record,
                    length: u64::MAX,
                }
            }
            1 => {
                changed[0].shape = StructuralTypeShape::FixedArray {
                    element: array,
                    length: 3,
                }
            }
            2 => {
                changed[0].shape = StructuralTypeShape::FixedArray {
                    element: borrowed_view,
                    length: 3,
                }
            }
            3 => changed[1].id = StructuralTypeId::new(99).unwrap(),
            4 => changed.push(changed[1].clone()),
            _ => {
                changed[1].shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    IntegerType::address(64).unwrap(),
                ))
            }
        }
        assert_eq!(shape(array, &changed), None, "changed geometry {mutation}");
    }
}

#[test]
fn inline_array_arguments_do_not_admit_stack_or_indirect_results() {
    use crate::selection::aggregate_result_input::{direct_fragments, inline_argument_fragments};
    for (policy, prefix) in [
        (CallingPolicy::SystemVAMD64, 6),
        (CallingPolicy::Aapcs64, 8),
        (CallingPolicy::MicrosoftX64, 4),
    ] {
        for bytes in 1_u32..=24 {
            let shape = ValueShape::integer(bytes, 1);
            let call = evaluate_call_plan(
                policy,
                &CallSignature {
                    parameters: vec![ValueShape::integer(8, 8); prefix]
                        .into_iter()
                        .chain([shape])
                        .collect(),
                    result: None,
                },
            )
            .unwrap();
            let placement = &call.parameters[prefix];
            let inline = match policy {
                CallingPolicy::SystemVAMD64 => true,
                CallingPolicy::Aapcs64 => bytes <= 16,
                CallingPolicy::MicrosoftX64 => matches!(bytes, 1 | 2 | 4 | 8),
                _ => unreachable!(),
            };
            assert_eq!(
                inline_argument_fragments(placement),
                inline,
                "{policy:?} {bytes}"
            );
            assert!(
                !direct_fragments(placement),
                "argument stack bytes are not result registers"
            );
        }
    }
    let mut placement = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(23, 1)],
            result: None,
        },
    )
    .unwrap()
    .parameters
    .remove(0);
    assert!(inline_argument_fragments(&placement));
    for mutation in 0..6 {
        let mut changed = placement.clone();
        let ValueLocation::Stack {
            stack_byte_offset,
            value_byte_offset,
            byte_size,
            alignment,
        } = &mut changed.locations[1]
        else {
            panic!("stack fragment");
        };
        match mutation {
            0 => *stack_byte_offset += 8,
            1 => *value_byte_offset += 1,
            2 => *byte_size = 0,
            3 => *byte_size = 9,
            4 => *alignment = 0,
            5 => *alignment = 3,
            _ => unreachable!(),
        }
        assert!(!inline_argument_fragments(&changed), "mutation {mutation}");
    }
    placement.locations.pop();
    assert!(!inline_argument_fragments(&placement));
}
