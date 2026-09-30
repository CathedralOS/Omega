use super::*;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression,
    CheckedStorageRoot, ClosedScalarContractValue,
};
use semantic_vocabulary::{EdgeId, PlaceId, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape,
};

#[test]
fn reentered_entry_reads_invocation_and_header_places_in_their_own_scopes() {
    let structural_type = StructuralTypeId::new(1).unwrap();
    let invocation = StructuralParameterDeclaration {
        place: PlaceId::new(1).unwrap(),
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let header_parameter = StructuralParameterDeclaration {
        place: PlaceId::new(2).unwrap(),
        ..invocation.clone()
    };
    let structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::Bytes".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        }),
    }];
    let parameters = vec![(0, invocation)];
    let element_views = std::collections::BTreeMap::new();
    let views = scalar_contracts::ContractViewNamespace {
        parameters: &parameters,
        structural_types: &structural_types,
        element_views: &element_views,
    };
    let clauses = vec![Some(ClosedScalarContractValue::Predicate(
        CheckedBooleanExpression::IntegerComparison {
            kind: CheckedIntegerComparisonKind::LessOrEqual,
            left: Box::new(CheckedScalarExpression::StructuralParameterByteLength {
                root: CheckedStorageRoot::Parameter { index: 0 },
                path: Vec::new(),
            }),
            right: Box::new(CheckedScalarExpression::IntegerLiteral {
                literal: validation::land_integer_value(
                    &numerics::bignum::BigInt::from_u64(9),
                    checked_trees::types::PrimitiveType::U64,
                )
                .unwrap(),
            }),
        },
    ))];
    let state = PreparedStateRequirements {
        clauses,
        parameters: vec![(0, header_parameter)],
    };
    let expected_invocation = scalar_contracts::clauses(&state.clauses, &[], &[], &views)
        .unwrap()
        .unwrap();
    let header_views = scalar_contracts::ContractViewNamespace {
        parameters: &state.parameters,
        ..views
    };
    let expected_header = scalar_contracts::clauses(&state.clauses, &[], &[], &header_views)
        .unwrap()
        .unwrap();
    assert_ne!(expected_invocation, expected_header);
    let entry = BlockId::new(2).unwrap();
    let edge = EdgeId::new(1).unwrap();
    let mut blocks = vec![Block {
        id: entry,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Jump {
            edge,
            target: BlockId::new(1).unwrap(),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![terminal_psi::StructuralArgument {
                place: parameters[0].1.place,
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    }];
    blocks.push(Block {
        id: BlockId::new(1).unwrap(),
        parameters: Vec::new(),
        structural_parameters: vec![state.parameters[0].1.clone()],
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnUnit {
            edge: EdgeId::new(2).unwrap(),
            trivial_affine_discards: Vec::new(),
        },
    });
    let mut calls = CallEmissionContext {
        machine_ids: &[],
        requirement_counts: &[],
        boundaries: &[],
        next_obligation_identity: 10,
        obligation_limit: 20,
    };
    let mut requires = Vec::new();
    let invariants = emit(
        &[state],
        &[Vec::new()],
        &[Vec::new()],
        &[],
        &[],
        &views,
        MachineId::new(1).unwrap(),
        0,
        entry,
        &blocks,
        &mut requires,
        &mut calls,
    )
    .unwrap();
    assert_eq!(requires, vec![expected_invocation]);
    assert_eq!(invariants.len(), 1);
    assert_eq!(invariants[0].predicate, expected_header);
    assert_eq!(invariants[0].arrivals.len(), 1);
    assert_eq!(invariants[0].arrivals[0].edge, edge);
}
