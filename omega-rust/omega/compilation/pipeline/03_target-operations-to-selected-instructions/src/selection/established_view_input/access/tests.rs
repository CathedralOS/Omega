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
    assert_eq!(
        element_view_access(&function, place(302)),
        Some(A::MutableBorrow)
    );
    assert_eq!(
        element_view_access(&function, place(303)),
        Some(A::SharedBorrow)
    );
    function.blocks.last_mut().unwrap().structural_parameters[0].access = A::SharedBorrow;
    assert_eq!(element_view_access(&function, place(302)), None);
    assert_eq!(
        element_view_access(&function, place(303)),
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
            element_view_access(&function, place(302)),
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
