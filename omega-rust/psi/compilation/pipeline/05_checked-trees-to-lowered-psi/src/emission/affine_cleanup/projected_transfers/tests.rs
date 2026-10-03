use super::*;
use crate::terminal_identities::{block_id, edge_id, place_id, value_id};
use semantic_vocabulary::{OperationId, StructuralFieldId};
use terminal_psi::{
    BindingRelevance, Operation, OperationResult, RecordFieldInitializer,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralTypeShape, SuccessorEdge,
};

fn identity(number: u64) -> StructuralTypeId {
    StructuralTypeId::new(number).unwrap()
}
fn types() -> Vec<StructuralTypeDeclaration> {
    vec![
        StructuralTypeDeclaration {
            id: identity(1),
            identity: "Root".into(),
            shape: StructuralTypeShape::Record {
                fields: ["left", "right"]
                    .iter()
                    .enumerate()
                    .map(|(position, name)| StructuralFieldDeclaration {
                        id: StructuralFieldId::new(position as u64 + 1).unwrap(),
                        identity: (*name).into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(identity(2)),
                    })
                    .collect(),
            },
        },
        StructuralTypeDeclaration {
            id: identity(2),
            identity: "Child".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
    ]
}
fn argument(name: &str) -> StructuralArgument {
    StructuralArgument {
        place: place_id(1),
        path: vec![StructuralPathSegment::Field(name.into())],
        access: StructuralAccess::Owned,
    }
}
fn construct(number: u64, fields: Vec<RecordFieldInitializer>) -> Operation {
    Operation {
        id: OperationId::new(number).unwrap(),
        static_reach_binding: None,
        suspension_crossing: None,
        kind: OperationKind::EstablishRecord { fields },
        result: OperationResult::Structural(StructuralOperationResult {
            place: place_id(number),
            structural_type: identity(if number == 1 { 1 } else { 2 }),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        }),
    }
}
fn projected() -> Operation {
    construct(
        2,
        vec![RecordFieldInitializer {
            field: StructuralFieldId::new(1).unwrap(),
            value: RecordFieldValue::Structural(argument("right")),
        }],
    )
}
fn jump(number: u64, target: u64) -> Block {
    Block {
        id: block_id(number),
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Jump {
            edge: edge_id(number),
            target: block_id(target),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    }
}
fn graph() -> Vec<Block> {
    let mut blocks = vec![jump(1, 2), jump(2, 3), jump(3, 4), jump(4, 5)];
    blocks[0].operations.push(construct(1, Vec::new()));
    blocks[1].operations.push(projected());
    blocks[3].terminator = Terminator::ReturnUnit {
        edge: edge_id(4),
        trivial_affine_discards: vec![place_id(1)],
    };
    blocks
}

#[test]
fn projected_cleanup_keeps_residual_alive_until_scheduled_exit() {
    let mut blocks = graph();
    super::complete(&[], block_id(1), &mut blocks, &types()).unwrap();
    for block in &blocks[..3] {
        let Terminator::Jump {
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } = &block.terminator
        else {
            panic!("jump");
        };
        assert!(trivial_affine_discards.is_empty() && residual_affine_discards.is_empty());
    }
    let Terminator::ReturnUnitPartialAffine {
        trivial_affine_discards,
        residual_affine_discards,
        ..
    } = &blocks[3].terminator
    else {
        panic!("partial return");
    };
    assert!(trivial_affine_discards.is_empty());
    assert_eq!(
        residual_affine_discards,
        &[StructuralAffineDiscard {
            place: place_id(1),
            path: vec![StructuralPathSegment::Field("left".into())],
            structural_type: identity(2)
        }]
    );
    let mut wrong = graph();
    wrong[2].operations.push(projected());
    assert!(super::complete(&[], block_id(1), &mut wrong, &types()).is_err());
}

#[test]
fn projected_cleanup_rejects_different_incoming_transfer_histories() {
    let mut blocks = graph();
    let edge = |number, target| SuccessorEdge {
        edge: edge_id(number),
        target: block_id(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    blocks[0].terminator = Terminator::Conditional {
        condition: value_id(1),
        when_true: edge(10, 2),
        when_false: edge(11, 3),
    };
    let before = blocks.clone();
    assert!(super::complete(&[], block_id(1), &mut blocks, &types()).is_err());
    assert_eq!(blocks, before, "failed completion is transactional");
}

#[test]
fn projected_cleanup_new_birth_does_not_inherit_old_moves() {
    let mut blocks = graph();
    blocks[2].operations.push(construct(1, Vec::new()));
    super::complete(&[], block_id(1), &mut blocks, &types()).unwrap();
    assert!(
        matches!(&blocks[3].terminator, Terminator::ReturnUnit { trivial_affine_discards, .. } if trivial_affine_discards == &[place_id(1)])
    );
}

#[test]
fn projected_cleanup_covering_transfer_joins_disposed_complement() {
    let mut blocks = graph();
    let edge = |number, target| SuccessorEdge {
        edge: edge_id(number),
        target: block_id(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    blocks[0].terminator = Terminator::Conditional {
        condition: value_id(1),
        when_true: edge(10, 2),
        when_false: edge(11, 3),
    };
    blocks[1].operations.push(construct(
        3,
        vec![RecordFieldInitializer {
            field: StructuralFieldId::new(1).unwrap(),
            value: RecordFieldValue::Structural(argument("left")),
        }],
    ));
    if let Terminator::Jump { target, .. } = &mut blocks[1].terminator {
        *target = block_id(4);
    }
    blocks[2].operations.push(projected());
    if let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &mut blocks[2].terminator
    {
        trivial_affine_discards.push(place_id(1));
    }
    blocks[3].terminator = Terminator::ReturnUnit {
        edge: edge_id(4),
        trivial_affine_discards: Vec::new(),
    };
    super::complete(&[], block_id(1), &mut blocks, &types()).unwrap();
}

#[test]
fn projected_cleanup_geometry_bounds_large_arrays_before_enumeration() {
    let mut declarations = types();
    declarations[0].shape = StructuralTypeShape::FixedArray {
        element: identity(2),
        length: u64::MAX,
    };
    let moved = BTreeSet::from([vec![StructuralPathSegment::FixedIndex(0)]]);
    assert!(
        terminal_semantics::partial_affine_residuals(&declarations, identity(1), &moved, 1)
            .is_none()
    );
    declarations[0].shape = StructuralTypeShape::FixedArray {
        element: identity(2),
        length: 3,
    };
    assert_eq!(
        terminal_semantics::partial_affine_residuals(&declarations, identity(1), &moved, 2)
            .unwrap(),
        vec![
            (vec![StructuralPathSegment::FixedIndex(2)], identity(2)),
            (vec![StructuralPathSegment::FixedIndex(1)], identity(2)),
        ]
    );
}
#[test]
fn projected_cleanup_orders_mixed_roots_by_birth_and_dominance() {
    let parameter = |place, position| StructuralParameterDeclaration {
        place: place_id(place),
        position,
        is_self: false,
        structural_type: identity(1),
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let selected = |number, source| {
        let mut operand = argument("right");
        operand.place = place_id(source);
        construct(
            number,
            vec![RecordFieldInitializer {
                field: StructuralFieldId::new(1).unwrap(),
                value: RecordFieldValue::Structural(operand),
            }],
        )
    };
    let born = |number| {
        let mut operation = construct(number, Vec::new());
        let OperationResult::Structural(result) = &mut operation.result else {
            unreachable!()
        };
        result.structural_type = identity(1);
        operation
    };
    let mut blocks = vec![jump(1, 30), jump(30, 5), jump(5, 6), jump(6, 7)];
    blocks[0].operations = vec![born(1), born(8)];
    if let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut blocks[0].terminator
    {
        structural_arguments.push(StructuralArgument {
            place: place_id(8),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        });
    }
    blocks[1].structural_parameters = vec![parameter(10, 0)];
    blocks[1].operations = vec![born(9)];
    if let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut blocks[1].terminator
    {
        structural_arguments.push(StructuralArgument {
            place: place_id(9),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        });
    }
    blocks[2].structural_parameters = vec![parameter(20, 0)];
    blocks[2].operations = vec![
        selected(2, 1),
        selected(3, 10),
        selected(4, 20),
        selected(7, 50),
    ];
    if let Terminator::Jump {
        trivial_affine_discards,
        residual_affine_discards,
        ..
    } = &mut blocks[2].terminator
    {
        trivial_affine_discards.extend([place_id(1), place_id(20), place_id(50)]);
        residual_affine_discards.push(StructuralAffineDiscard {
            place: place_id(10),
            path: vec![StructuralPathSegment::Field("left".into())],
            structural_type: identity(2),
        });
    }
    blocks[3].terminator = Terminator::ReturnUnit {
        edge: edge_id(6),
        trivial_affine_discards: Vec::new(),
    };
    // Serialization order and block IDs both disagree with the live parameter
    // establishment order. Only dominance gives the receiving cleanup order.
    blocks.swap(1, 2);
    super::complete(&[parameter(50, 0)], block_id(1), &mut blocks, &types()).unwrap();
    let Terminator::Jump {
        residual_affine_discards,
        trivial_affine_discards,
        ..
    } = &blocks[1].terminator
    else {
        panic!("cleanup edge")
    };
    assert!(trivial_affine_discards.is_empty());
    assert_eq!(
        residual_affine_discards
            .iter()
            .map(|discard| discard.place)
            .collect::<Vec<_>>(),
        [place_id(1), place_id(20), place_id(10), place_id(50)]
    );
}
