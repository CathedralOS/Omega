use super::*;
use semantic_vocabulary::{ObligationId, OperationId, StructuralTypeId, ValueId};
use terminal_psi::{
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration,
};

fn place(number: u64) -> PlaceId {
    PlaceId::new(number).unwrap()
}
fn operation(number: u64) -> OperationId {
    OperationId::new(number).unwrap()
}
fn views() -> LegalizedScalarFunction {
    let mut function = crate::tests::legalization::owned_block_store::legalized_arrival();
    let template = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .next()
        .unwrap()
        .clone();
    let owner = function.blocks.last().unwrap().id;
    for block in &mut function.blocks {
        block.structural_parameters.clear();
        block.instructions.clear();
    }
    let view = StructuralTypeId::new(301).unwrap();
    let element = StructuralTypeId::new(302).unwrap();
    let parameter = StructuralParameterDeclaration {
        place: place(301),
        position: 0,
        is_self: false,
        structural_type: view,
        multiplicity: M::Unrestricted,
        access: A::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let signature = function.structural.as_mut().unwrap();
    signature.parameters.clear();
    signature.structural_types = vec![
        StructuralTypeDeclaration {
            id: view,
            identity: "View".into(),
            shape: StructuralTypeShape::ElementView { element },
        },
        StructuralTypeDeclaration {
            id: element,
            identity: "Element".into(),
            shape: StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Boolean),
        },
    ]
    .into();
    signature.structural_places = vec![StructuralPlaceDeclaration {
        id: place(301),
        kind: StructuralPlaceKind::BlockParameter {
            block: owner,
            position: 0,
        },
    }];
    function
        .blocks
        .last_mut()
        .unwrap()
        .structural_parameters
        .push(parameter);
    for (number, source, multiplicity) in [(302, 301, M::Affine), (303, 302, M::Unrestricted)] {
        let mut instruction = template.clone();
        instruction.operation = operation(number);
        instruction.result = None;
        instruction.kind = I::ElementViewSubslice {
            result: StructuralOperationResult {
                place: place(number),
                structural_type: view,
                multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            },
            source: place(source),
            start: ValueId::new(1).unwrap(),
            end: ValueId::new(2).unwrap(),
            length: ValueId::new(3).unwrap(),
            obligation: ObligationId::new(number).unwrap(),
            accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([0; 32]),
        };
        function
            .blocks
            .last_mut()
            .unwrap()
            .instructions
            .push(instruction);
        function
            .structural
            .as_mut()
            .unwrap()
            .structural_places
            .push(StructuralPlaceDeclaration {
                id: place(number),
                kind: StructuralPlaceKind::OperationResult {
                    producer: operation(number),
                    structural_type: view,
                },
            });
    }
    function
}

#[test]
fn mutable_subslice_selection_replays_parent_permission_and_attenuation() {
    let mut function = views();
    assert_eq!(view_access(&function, place(302)), Some(A::MutableBorrow));
    assert_eq!(view_access(&function, place(303)), Some(A::SharedBorrow));
    function.blocks.last_mut().unwrap().structural_parameters[0].access = A::SharedBorrow;
    assert_eq!(view_access(&function, place(302)), None);
    assert_eq!(
        view_access(&function, place(303)),
        None,
        "attenuation cannot conceal an invalid ancestor"
    );
}

#[test]
fn mutable_subslice_selection_rejects_source_type_producer_and_cycle_substitution() {
    for mutation in 0..4 {
        let mut function = views();
        let row = &mut function.blocks.last_mut().unwrap().instructions[0];
        let I::ElementViewSubslice { result, source, .. } = &mut row.kind else {
            panic!()
        };
        match mutation {
            0 => *source = place(999),
            1 => result.structural_type = StructuralTypeId::new(302).unwrap(),
            2 => row.operation = operation(999),
            3 => *source = place(303),
            _ => unreachable!(),
        }
        assert_eq!(
            view_access(&function, place(302)),
            None,
            "mutation {mutation}"
        );
    }
}

#[test]
fn mutable_view_call_receiver_preserves_exact_whole_descriptor_authority() {
    use calling_conventions::{CallSignature, ValueShape, evaluate_call_plan};
    use target_operations::{TargetStructuralArgument, TargetStructuralArgumentSource};

    let function = views();
    let shape = ValueShape::borrowed_reference(16, 8);
    let call = evaluate_call_plan(
        function.call_plan.policy,
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .unwrap();
    let argument = TargetStructuralArgument {
        place: place(302),
        access: A::MutableBorrow,
        path: Vec::new(),
        root_structural_type: StructuralTypeId::new(301).unwrap(),
        structural_type: StructuralTypeId::new(301).unwrap(),
        shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        indices: Vec::new(),
        source: TargetStructuralArgumentSource::EstablishedElementView {
            psi_operation: operation(302),
        },
        destination: call.parameters[0].clone(),
    };
    // The next retained occurrence supplies the use position. Full graph
    // admission separately checks the actual call and its formal signature.
    assert!(super::super::accepts(&function, operation(303), &argument).is_some());
    for mutation in 0..3 {
        let mut changed = function.clone();
        let mut actual = argument.clone();
        match mutation {
            0 => {
                changed.blocks.last_mut().unwrap().structural_parameters[0].access = A::SharedBorrow
            }
            1 => actual.access = A::Owned,
            2 => {
                actual.source = TargetStructuralArgumentSource::EstablishedElementView {
                    psi_operation: operation(999),
                }
            }
            _ => unreachable!(),
        }
        assert!(
            super::super::accepts(&changed, operation(303), &actual).is_none(),
            "receiver mutation {mutation}"
        );
    }
}

#[test]
fn indexed_byte_field_root_cannot_promote_descriptor_custody_to_backing_access() {
    let mut function = views();
    let mut argument = terminal_psi::StructuralArgument {
        place: place(302),
        path: Vec::new(),
        access: A::MutableBorrow,
    };
    assert_eq!(
        crate::selection::field_bytes::root_type(&function, &argument),
        Some(StructuralTypeId::new(301).unwrap())
    );
    argument.access = A::Owned;
    assert_eq!(
        crate::selection::field_bytes::root_type(&function, &argument),
        None
    );
    argument.access = A::MutableBorrow;
    function.blocks.last_mut().unwrap().structural_parameters[0].access = A::SharedBorrow;
    assert_eq!(
        crate::selection::field_bytes::root_type(&function, &argument),
        None,
        "an affine child descriptor cannot manufacture exclusive backing permission"
    );
}

fn byte_views() -> LegalizedScalarFunction {
    let mut function = views();
    let signature = function.structural.as_mut().unwrap();
    let mut types = signature.structural_types.to_vec();
    types[0].shape =
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: None,
        });
    types[1].shape =
        StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .unwrap(),
        ));
    signature.structural_types = types.into();
    for row in &mut function.blocks.last_mut().unwrap().instructions {
        let I::ElementViewSubslice {
            result,
            source,
            start,
            end,
            length,
            obligation,
            accepted_fact,
        } = row.kind.clone()
        else {
            panic!("fixture must contain subslices")
        };
        row.kind = I::ByteSequenceSubslice {
            result,
            source,
            start,
            end,
            length,
            obligation,
            accepted_fact,
        };
    }
    function
}

#[test]
fn mutable_byte_subslice_selection_replays_parent_and_result_custody() {
    let function = byte_views();
    assert_eq!(view_access(&function, place(302)), Some(A::MutableBorrow));
    assert_eq!(view_access(&function, place(303)), Some(A::SharedBorrow));
    for mutation in 0..6 {
        let mut changed = function.clone();
        let block = changed.blocks.last_mut().unwrap();
        let row = &mut block.instructions[0];
        let I::ByteSequenceSubslice { result, source, .. } = &mut row.kind else {
            panic!()
        };
        match mutation {
            0 => block.structural_parameters[0].access = A::SharedBorrow,
            1 => *source = place(999),
            2 => result.structural_type = StructuralTypeId::new(302).unwrap(),
            3 => row.operation = operation(999),
            4 => *source = place(303),
            5 => result.multiplicity = M::Linear,
            _ => unreachable!(),
        }
        assert_eq!(
            view_access(&changed, place(302)),
            None,
            "mutation {mutation}"
        );
        assert_eq!(
            view_access(&changed, place(303)),
            None,
            "invalid ancestor {mutation}"
        );
    }
}

#[test]
fn mutable_byte_subslice_call_replays_exact_receiver_permission() {
    use calling_conventions::{CallSignature, ValueShape, evaluate_call_plan};
    use target_operations::{TargetStructuralArgument, TargetStructuralArgumentSource};
    let function = byte_views();
    let shape = ValueShape::borrowed_reference(16, 8);
    let call = evaluate_call_plan(
        function.call_plan.policy,
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .unwrap();
    let argument = TargetStructuralArgument {
        place: place(302),
        access: A::MutableBorrow,
        path: Vec::new(),
        root_structural_type: StructuralTypeId::new(301).unwrap(),
        structural_type: StructuralTypeId::new(301).unwrap(),
        shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        indices: Vec::new(),
        source: TargetStructuralArgumentSource::EstablishedByteView {
            psi_operation: operation(302),
        },
        destination: call.parameters[0].clone(),
    };
    assert!(super::super::accepts(&function, operation(303), &argument).is_some());
    for mutation in 0..4 {
        let mut changed = function.clone();
        let mut actual = argument.clone();
        match mutation {
            0 => {
                changed.blocks.last_mut().unwrap().structural_parameters[0].access = A::SharedBorrow
            }
            1 => actual.access = A::Owned,
            2 => {
                actual.source = TargetStructuralArgumentSource::EstablishedByteView {
                    psi_operation: operation(999),
                }
            }
            3 => {
                actual.source = TargetStructuralArgumentSource::EstablishedElementView {
                    psi_operation: operation(302),
                }
            }
            _ => unreachable!(),
        }
        assert!(
            super::super::accepts(&changed, operation(303), &actual).is_none(),
            "receiver mutation {mutation}"
        );
    }
}

#[test]
fn shared_qualified_byte_subslice_preserves_read_only_source_permission() {
    let mut function = byte_views();
    let block = function.blocks.last_mut().unwrap();
    block.structural_parameters[0].access = A::SharedBorrow;
    block.structural_parameters[0]
        .qualifications
        .push(semantic_vocabulary::StructuralDomainId::new(301).unwrap());
    let I::ByteSequenceSubslice { result, .. } = &mut block.instructions[0].kind else {
        panic!()
    };
    result.multiplicity = M::Unrestricted;
    assert_eq!(view_access(&function, place(302)), Some(A::SharedBorrow));
    assert_eq!(view_access(&function, place(303)), Some(A::SharedBorrow));
    function.blocks.last_mut().unwrap().structural_parameters[0].access = A::MutableBorrow;
    assert_eq!(
        view_access(&function, place(302)),
        None,
        "mutable qualification preservation needs independent store evidence"
    );
}

fn copied_byte_field() -> LegalizedScalarFunction {
    let mut function = byte_views();
    let container = StructuralTypeId::new(304).unwrap();
    let signature = function.structural.as_mut().unwrap();
    let mut types = signature.structural_types.to_vec();
    types.push(StructuralTypeDeclaration {
        id: container,
        identity: "Remainder".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![terminal_psi::StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(304).unwrap(),
                identity: "bytes".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: terminal_psi::StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(A::SharedBorrow),
                    },
                ),
            }],
        },
    });
    signature.structural_types = types.into();
    let block = function.blocks.last_mut().unwrap();
    block.structural_parameters[0].structural_type = container;
    block.structural_parameters[0].access = A::Owned;
    block.structural_parameters[0].multiplicity = M::Affine;
    let I::ByteSequenceSubslice { mut result, .. } = block.instructions[0].kind.clone() else {
        panic!()
    };
    result.multiplicity = M::Unrestricted;
    block.instructions[0].kind = I::StructuralLeafCopy {
        result,
        source: place(301),
        path: abstract_operations::StructuralCopyPath::Ordinary(vec![
            terminal_psi::StructuralPathSegment::Field("bytes".into()),
        ]),
        byte_offset: 0,
        shape: calling_conventions::ValueShape::borrowed_reference(16, 8),
        indices: Vec::new(),
    };
    function
}

#[test]
fn copied_descriptor_selection_replays_field_permission_and_following_subslice() {
    let function = copied_byte_field();
    assert_eq!(view_access(&function, place(302)), Some(A::SharedBorrow));
    assert_eq!(view_access(&function, place(303)), Some(A::SharedBorrow));
    for mutation in 0..8 {
        let mut changed = function.clone();
        let block = changed.blocks.last_mut().unwrap();
        let row = &mut block.instructions[0];
        let I::StructuralLeafCopy {
            result,
            source,
            path,
            byte_offset,
            ..
        } = &mut row.kind
        else {
            panic!()
        };
        match mutation {
            0 => *source = place(999),
            1 => {
                *path = abstract_operations::StructuralCopyPath::Ordinary(vec![
                    terminal_psi::StructuralPathSegment::Field("other".into()),
                ])
            }
            2 => *byte_offset = 8,
            3 => result.multiplicity = M::Affine,
            4 => block.structural_parameters[0].access = A::WriteOnlyBorrow,
            5 => row.operation = operation(999),
            6 => {
                let signature = changed.structural.as_mut().unwrap();
                let mut types = signature.structural_types.to_vec();
                let StructuralTypeShape::Record { fields } = &mut types[2].shape else {
                    panic!()
                };
                fields[0].field_type = terminal_psi::StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(A::MutableBorrow),
                    },
                );
                signature.structural_types = types.into();
            }
            7 => result.structural_type = StructuralTypeId::new(302).unwrap(),
            _ => unreachable!(),
        }
        assert_eq!(
            view_access(&changed, place(302)),
            None,
            "mutation {mutation}"
        );
        assert_eq!(
            view_access(&changed, place(303)),
            None,
            "ancestor mutation {mutation}"
        );
    }
}

#[test]
fn copied_descriptor_selection_cannot_copy_mutable_whole_authority() {
    let mut function = byte_views();
    let row = &mut function.blocks.last_mut().unwrap().instructions[0];
    let I::ByteSequenceSubslice {
        mut result, source, ..
    } = row.kind.clone()
    else {
        panic!()
    };
    result.multiplicity = M::Unrestricted;
    row.kind = I::StructuralLeafCopy {
        result,
        source,
        path: abstract_operations::StructuralCopyPath::Ordinary(Vec::new()),
        byte_offset: 0,
        shape: calling_conventions::ValueShape::borrowed_reference(16, 8),
        indices: Vec::new(),
    };
    assert_eq!(view_access(&function, place(302)), None);
    function.blocks.last_mut().unwrap().structural_parameters[0].access = A::SharedBorrow;
    assert_eq!(view_access(&function, place(302)), Some(A::SharedBorrow));
    for mutation in 0..3 {
        let mut changed = function.clone();
        let I::StructuralLeafCopy {
            byte_offset,
            shape,
            indices,
            ..
        } = &mut changed.blocks.last_mut().unwrap().instructions[0].kind
        else {
            panic!()
        };
        match mutation {
            0 => *byte_offset = 8,
            1 => shape.byte_size = 8,
            2 => indices.push(legalized_operations::LegalizedRuntimeIndexOperand {
                operand: abstract_operations::AbstractResult {
                    value: ValueId::new(999).unwrap(),
                    scalar_type: semantic_vocabulary::ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            64,
                        )
                        .unwrap(),
                    ),
                },
                stride: 16,
                extent: Some(1),
                obligation: ObligationId::new(999).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [0; 32],
                ),
            }),
            _ => unreachable!(),
        }
        assert_eq!(
            view_access(&changed, place(302)),
            None,
            "whole geometry {mutation}"
        );
    }
    let I::StructuralLeafCopy { source, .. } =
        &mut function.blocks.last_mut().unwrap().instructions[0].kind
    else {
        panic!()
    };
    *source = place(302);
    assert_eq!(view_access(&function, place(302)), None);
}
