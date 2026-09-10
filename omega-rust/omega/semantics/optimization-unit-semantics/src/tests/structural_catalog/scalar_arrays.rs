//! Whole-array custody and leaf identity survive ordinary construction and calls.

use super::super::*;
use abstract_operations::AbstractOperation as O;
use terminal_psi::{StructuralMultiplicity, StructuralTypeShape};

fn array_unit(length: u64) -> PsiOptimizationUnit {
    let mut candidate = structural_result_call_unit();
    let primitive = id(30_001, StructuralTypeId::new);
    let structural_type = candidate.structural_types[0].id;
    candidate.structural_types[0].shape = StructuralTypeShape::FixedArray {
        element: primitive,
        length,
    };
    candidate
        .structural_types
        .push(terminal_psi::StructuralTypeDeclaration {
            id: primitive,
            identity: "test::array-leaf".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        });
    for function in &mut candidate.functions {
        function.verified_contract = Some(terminal_psi::MachineContract {
            id: id(30_006, semantic_vocabulary::ContractId::new),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        });
        function.entry_claim_declarations.clear();
        function.entry_claims.clear();
        for parameter in &mut function.structural_parameters {
            parameter.multiplicity = StructuralMultiplicity::Unrestricted;
        }
        if let abstract_operations::AbstractFunctionResult::Structural(result) =
            &mut function.result
        {
            result.multiplicity = StructuralMultiplicity::Unrestricted;
        }
        for node in function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.nodes)
        {
            match &mut node.operation {
                O::CallStructural {
                    result,
                    claim_transfers,
                    returned_claim_transfers,
                    ..
                } => {
                    result.multiplicity = StructuralMultiplicity::Unrestricted;
                    result.claims.clear();
                    claim_transfers.clear();
                    returned_claim_transfers.clear();
                }
                O::ReturnStructural {
                    returned_claims, ..
                } => returned_claims.clear(),
                _ => {}
            }
        }
    }
    let place = id(30_002, PlaceId::new);
    let operation = id(30_003, OperationId::new);
    let value = id(30_004, ValueId::new);
    let function = &mut candidate.functions[0];
    function
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type,
            },
        });
    let mut constant = function.blocks[0].nodes[0].clone();
    constant.operation = O::BooleanConstant {
        psi_operation: id(30_005, OperationId::new),
        result: value,
        value: true,
    };
    let mut establishment = constant.clone();
    establishment.operation = O::EstablishScalarArray {
        psi_operation: operation,
        result: terminal_psi::StructuralOperationResult {
            place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        elements: vec![value; usize::try_from(length).unwrap()],
    };
    let O::CallStructural {
        structural_arguments,
        ..
    } = &mut function.blocks[0].nodes[0].operation
    else {
        panic!("fixture call")
    };
    structural_arguments[0].place = place;
    function.blocks[0].nodes.insert(0, establishment);
    function.blocks[0].nodes.insert(0, constant);
    function.declared_places.insert(place);
    for index in 0..candidate.functions.len() {
        refresh_function_derivatives(&mut candidate, index);
    }
    candidate
}

#[test]
fn scalar_array_repeated_leaves_pass_and_return_through_owned_calls() {
    for length in [0, 2] {
        let candidate = array_unit(length);
        assert_eq!(validate_psi_optimization_unit(&candidate), Ok(()));
        let uses = &candidate.functions[0].blocks[0].nodes[1].uses;
        assert_eq!(uses.len(), usize::try_from(length).unwrap());
        if length != 0 {
            assert_eq!(uses[0].value, uses[1].value);
        }
    }
}

#[test]
fn scalar_array_corrupt_payload_or_custody_rejects_after_metadata_refresh() {
    for mutation in 0..11 {
        let mut candidate = array_unit(2);
        let O::EstablishScalarArray {
            result, elements, ..
        } = &mut candidate.functions[0].blocks[0].nodes[1].operation
        else {
            panic!("array")
        };
        match mutation {
            0 => {
                elements.pop();
            }
            1 => elements[0] = id(30_099, ValueId::new),
            2 => result.multiplicity = StructuralMultiplicity::Affine,
            3 => result.place = id(30_098, PlaceId::new),
            4 => {
                candidate.structural_types[1].shape =
                    StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            8,
                        )
                        .unwrap(),
                    ))
            }
            5 => candidate.functions[0].blocks[0].nodes.swap(0, 1),
            6 => {
                candidate.functions[0]
                    .structural_places
                    .last_mut()
                    .unwrap()
                    .kind = StructuralPlaceKind::OperationResult {
                    producer: id(30_097, OperationId::new),
                    structural_type: candidate.structural_types[0].id,
                }
            }
            7 => result
                .claims
                .push(terminal_psi::StructuralResultClaimBinding {
                    claim: id(30_090, semantic_vocabulary::ClaimId::new),
                    path: Vec::new(),
                }),
            8 => result
                .qualifications
                .push(id(30_091, semantic_vocabulary::StructuralDomainId::new)),
            9 => result
                .projected_qualifications
                .push(terminal_psi::StructuralPathQualification {
                    path: vec![terminal_psi::StructuralPathSegment::FixedIndex(0)],
                    domain: id(30_091, semantic_vocabulary::StructuralDomainId::new),
                }),
            10 => candidate.functions[0].blocks[0].nodes.swap(1, 2),
            _ => unreachable!(),
        }
        refresh_function_derivatives(&mut candidate, 0);
        assert!(
            validate_psi_optimization_unit(&candidate).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn scalar_array_empty_outer_dimension_still_validates_inner_carrier() {
    let mut candidate = array_unit(0);
    let inner = id(30_010, StructuralTypeId::new);
    let primitive = candidate.structural_types[1].id;
    candidate.structural_types[0].shape = StructuralTypeShape::FixedArray {
        element: inner,
        length: 0,
    };
    candidate
        .structural_types
        .push(terminal_psi::StructuralTypeDeclaration {
            id: inner,
            identity: "test::inner-array".into(),
            shape: StructuralTypeShape::FixedArray {
                element: primitive,
                length: 3,
            },
        });
    refresh_identity(&mut candidate);
    assert_eq!(validate_psi_optimization_unit(&candidate), Ok(()));
    for mutation in 0..3 {
        let mut changed = candidate.clone();
        changed.structural_types[2].shape = match mutation {
            0 => StructuralTypeShape::FixedArray {
                element: id(30_099, StructuralTypeId::new),
                length: 0,
            },
            1 => StructuralTypeShape::FixedArray {
                element: changed.structural_types[0].id,
                length: 0,
            },
            2 => StructuralTypeShape::Record { fields: Vec::new() },
            _ => unreachable!(),
        };
        refresh_identity(&mut changed);
        assert!(
            validate_psi_optimization_unit(&changed).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn scalar_array_rewrites_preserve_every_leaf_occurrence_and_result_identity() {
    let candidate = array_unit(2);
    let original = candidate.functions[0].blocks[0].nodes[1].operation.clone();
    let from = id(30_004, ValueId::new);
    let to = id(30_020, ValueId::new);
    let mut expected = original.clone();
    let O::EstablishScalarArray { elements, .. } = &mut expected else {
        panic!("array")
    };
    *elements = vec![to, to];
    let mut substitution = original.clone();
    crate::candidates::rewrite_scalar_value_uses(&mut substitution, from, to);
    assert_eq!(substitution, expected);
    let mut parameter_rewrite = original;
    crate::candidates::rewrite_block_parameter_operation(
        &mut parameter_rewrite,
        optimization_unit::RedundantBlockParameterRewrite {
            machine: candidate.functions[0].machine,
            block: candidate.functions[0].entry,
            position: 0,
            parameter: from,
            replacement: to,
            scalar_type: ScalarType::Boolean,
        },
    );
    assert_eq!(parameter_rewrite, expected);
}
