//! A bounded inline byte field presents as a borrowed view: the staged
//! descriptor reads the field's live length word, never its declared capacity.
use super::{
    CallSignature, CallingPolicy, IntegerSign, IntegerType, LegalizedScalarArgument,
    LegalizedScalarFunction, LegalizedScalarInstructionKind, ScalarType, SelectedFunction,
    SelectedInstructionKind, SelectedSelectionConstraints, StructuralAccess,
    StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, ValueShape, build,
    evaluate_call_plan,
};
use crate::selection::construction::scalar_graph::tests::borrowed_calls;

fn byte_field_call(native: target::NativeTarget) -> LegalizedScalarFunction {
    let mut source = borrowed_calls::borrowed_call(native);
    let view = StructuralTypeId::new(1).unwrap();
    let record = StructuralTypeId::new(2).unwrap();
    // `flag` occupies bytes 0..8; `out` begins at offset 8 as a live length word
    // followed by three capacity bytes, so the record spans 24 bytes aligned 8.
    let root_shape = ValueShape::borrowed_reference(24, 8);
    let view_shape = ValueShape::borrowed_reference(16, 8);
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(native),
        &CallSignature {
            parameters: vec![root_shape],
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap();
    let contract = source.structural.as_mut().unwrap();
    contract.structural_types = vec![
        StructuralTypeDeclaration {
            id: view,
            identity: "bytes".into(),
            shape: StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView,
            ),
        },
        StructuralTypeDeclaration {
            id: record,
            identity: "Printer".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                        identity: "flag".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        )),
                    },
                    StructuralFieldDeclaration {
                        id: semantic_vocabulary::StructuralFieldId::new(2).unwrap(),
                        identity: "out".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 3 },
                        ),
                    },
                ],
            },
        },
    ]
    .into();
    contract.parameters[0].semantic.structural_type = record;
    contract.parameters[0].semantic.access = StructuralAccess::MutableBorrow;
    contract.parameters[0].target.structural_type = record;
    contract.parameters[0].target.access = StructuralAccess::MutableBorrow;
    contract.parameters[0].target.shape = root_shape;
    contract.parameters[0].target.placement = source.call_plan.parameters[0].clone();
    let LegalizedScalarInstructionKind::Call(call) = &mut source.blocks[0].instructions[0].kind
    else {
        panic!("call");
    };
    call.call_plan = evaluate_call_plan(
        source.call_plan.policy,
        &CallSignature {
            parameters: vec![view_shape],
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap();
    call.result_placement = call.call_plan.result.clone();
    let LegalizedScalarArgument::Structural { semantic, target } = &mut call.arguments[0] else {
        panic!("receiver");
    };
    semantic.access = StructuralAccess::SharedBorrow;
    semantic.path = vec![StructuralPathSegment::Field("out".into())];
    target.access = semantic.access;
    target.path = semantic.path.clone();
    target.root_structural_type = record;
    target.structural_type = view;
    target.shape = view_shape;
    target.source_byte_offset = 8;
    target.source = source.call_plan.parameters[0].clone().into();
    target.destination = call.call_plan.parameters[0].clone();
    source
}

#[test]
fn bounded_byte_field_call_stages_live_length_descriptor() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let source = byte_field_call(native);
        let construct = |source: &LegalizedScalarFunction| {
            build(
                0,
                source,
                native,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        let selected = construct(&source).expect("bounded byte field call selects");
        let validate = |source: &LegalizedScalarFunction, candidate: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                source,
                candidate,
                native,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&source, &selected).unwrap();
        let place = semantic_vocabulary::PlaceId::new(1).unwrap();
        let slot = selected_instructions::LocalStorageSlotId::Structural {
            operation: source.blocks[0].instructions[0].operation,
            place,
        };
        // The field pointer is the root plus its authored offset; the data
        // pointer is eight bytes later, past the inline live length word.
        let field_offsets = selected.blocks[0]
            .instructions
            .iter()
            .filter(|instruction| {
                matches!(
                    instruction.kind,
                    SelectedInstructionKind::AddressOffset { byte_offset: 8 }
                )
            })
            .count();
        assert_eq!(
            field_offsets, 2,
            "field projection and data pointer each add their exact displacement"
        );
        assert!(selected.blocks[0].instructions.iter().any(|instruction| {
            matches!(
                instruction.kind,
                SelectedInstructionKind::Load64 { byte_offset: 0 }
            )
        }));
        for byte_offset in [0, 8] {
            assert!(selected.blocks[0].instructions.iter().any(|instruction| {
                matches!(
                    instruction.kind,
                    SelectedInstructionKind::Store64 {
                        slot: selected_instructions::FrameStorageSlotId::Local(candidate),
                        byte_offset: offset,
                    } if candidate == slot && offset == byte_offset
                )
            }));
        }
        assert!(selected.blocks[0].instructions.iter().any(|instruction| {
            matches!(
                instruction.kind,
                SelectedInstructionKind::FrameAddress {
                    slot: selected_instructions::FrameStorageSlotId::Local(candidate),
                    byte_offset: 0,
                } if candidate == slot
            )
        }));
        assert!(
            selected
                .local_storage_slots
                .iter()
                .any(|candidate| candidate.id == slot
                    && candidate.byte_size == 16
                    && candidate.alignment == 8)
        );
        // The live length word is the only field byte read; capacity is never
        // materialized and the referent bytes stay in the caller's record.
        assert_eq!(
            selected
                .memory_accesses
                .iter()
                .filter(|access| access.role
                    == selected_instructions::SelectedMemoryAccessRole::ReadPlace)
                .count(),
            1
        );
        assert!(selected.memory_accesses.iter().any(|access| {
            access.place == place
                && access.byte_offset == 8
                && access.byte_count == 8
                && access.role == selected_instructions::SelectedMemoryAccessRole::ReadPlace
        }));
        for byte_offset in [0, 8] {
            assert!(selected.memory_accesses.iter().any(|access| {
                access.byte_offset == byte_offset
                    && access.byte_count == 8
                    && access.role
                        == selected_instructions::SelectedMemoryAccessRole::WriteLocal { slot }
            }));
        }
        assert!(selected.memory_accesses.iter().any(|access| {
            access.byte_offset == 0
                && access.byte_count == 16
                && access.role
                    == selected_instructions::SelectedMemoryAccessRole::AddressLocal { slot }
        }));
        for mutation in 0..6 {
            let mut changed = selected.clone();
            match mutation {
                0 => {
                    changed.blocks[0]
                        .instructions
                        .iter_mut()
                        .find(|instruction| {
                            matches!(instruction.kind, SelectedInstructionKind::Load64 { .. })
                        })
                        .unwrap()
                        .kind = SelectedInstructionKind::Load64 { byte_offset: 8 }
                }
                1 => {
                    changed.blocks[0]
                        .instructions
                        .iter_mut()
                        .find(|instruction| {
                            matches!(
                                instruction.kind,
                                SelectedInstructionKind::FrameAddress { .. }
                            )
                        })
                        .unwrap()
                        .kind = SelectedInstructionKind::CopyI64
                }
                2 => changed.memory_accesses.retain(|access| {
                    access.role != selected_instructions::SelectedMemoryAccessRole::ReadPlace
                }),
                3 => {
                    let LegalizedScalarArgument::Structural { target, .. } =
                        &mut changed.calls[0].call.arguments[0]
                    else {
                        panic!("structural call")
                    };
                    target.source_byte_offset = 0
                }
                4 => {
                    let LegalizedScalarArgument::Structural { semantic, .. } =
                        &mut changed.calls[0].call.arguments[0]
                    else {
                        panic!("structural call")
                    };
                    semantic.path = vec![StructuralPathSegment::Field("flag".into())]
                }
                _ => {
                    let LegalizedScalarArgument::Structural { target, .. } =
                        &mut changed.calls[0].call.arguments[0]
                    else {
                        panic!("structural call")
                    };
                    target.structural_type = StructuralTypeId::new(2).unwrap()
                }
            }
            assert!(
                validate(&source, &changed).is_err(),
                "selected mutation {mutation} on {native:?}"
            );
        }
        for mutation in 0..5 {
            let mut changed = source.clone();
            let LegalizedScalarInstructionKind::Call(call) =
                &mut changed.blocks[0].instructions[0].kind
            else {
                panic!("call fixture")
            };
            let LegalizedScalarArgument::Structural { semantic, target } = &mut call.arguments[0]
            else {
                panic!("structural call")
            };
            match mutation {
                0 => target.source_byte_offset = 4,
                1 => semantic.path = vec![StructuralPathSegment::Field("flag".into())],
                2 => semantic.path = vec![StructuralPathSegment::Field("missing".into())],
                3 => target.structural_type = StructuralTypeId::new(2).unwrap(),
                _ => {
                    semantic.access = StructuralAccess::Owned;
                    target.access = semantic.access;
                }
            }
            assert!(
                construct(&changed).is_err() || validate(&changed, &selected).is_err(),
                "source mutation {mutation} on {native:?}"
            );
        }
    }
}
