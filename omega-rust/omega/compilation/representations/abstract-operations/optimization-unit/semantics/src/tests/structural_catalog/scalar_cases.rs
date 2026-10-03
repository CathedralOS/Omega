//! Constructor payload identity, scalar use and declaration corruption controls.
use crate::tests::fixtures::scalar_units::unit;
use crate::tests::support::{id, refresh_identity, refresh_node_derivatives};
use abstract_operations::AbstractOperation;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind, StructuralTypeId, ValueId};

fn scalar_case_unit() -> PsiOptimizationUnit {
    let mut candidate = unit();
    let structural_type = id(801, StructuralTypeId::new);
    let case = id(802, semantic_vocabulary::StructuralCaseId::new);
    let field = id(803, semantic_vocabulary::StructuralFieldId::new);
    let place = id(804, PlaceId::new);
    let operation = id(805, OperationId::new);
    let scalar_type = candidate.functions[0].blocks[0].nodes[0].definitions[0].scalar_type;
    candidate.structural_types = vec![terminal_psi::StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::ScalarOutcome".into(),
        shape: terminal_psi::StructuralTypeShape::Sum {
            cases: vec![terminal_psi::StructuralCaseDeclaration {
                id: case,
                identity: "Value".into(),
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: field,
                    identity: "value".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Scalar(scalar_type),
                }],
            }],
        },
    }]
    .into();
    candidate.functions[0]
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type,
            },
        });
    let mut node = candidate.functions[0].blocks[0].nodes[0].clone();
    node.operation = AbstractOperation::EstablishCase {
        psi_operation: operation,
        result: terminal_psi::StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place,
            structural_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        result_case: case,
        fields: vec![terminal_psi::RecordFieldInitializer {
            field,
            value: terminal_psi::RecordFieldValue::Scalar {
                value: id(3, ValueId::new),
                range_obligation: None,
            },
        }],
    };
    candidate.functions[0].blocks[0].nodes.insert(1, node);
    refresh_node_derivatives(&mut candidate, 0, 0, 1);
    refresh_node_derivatives(&mut candidate, 0, 0, 2);
    refresh_identity(&mut candidate);
    candidate
}

#[test]
fn scalar_case_initializer_is_a_live_typed_scalar_use() {
    let candidate = scalar_case_unit();
    let function = &candidate.functions[0];
    let node = &function.blocks[0].nodes[1];
    assert_eq!(node.uses.len(), 1);
    assert_eq!(node.uses[0].value, id(3, ValueId::new));
    let types = candidate
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect();
    assert!(
        crate::unit_validation::operation_contracts::case_establishment_matches(
            function,
            &node.operation,
            &types,
            &Default::default(),
        )
    );
    let original = candidate.identity;
    for mutation in 0..5 {
        let mut changed = candidate.clone();
        let AbstractOperation::EstablishCase {
            fields,
            result_case,
            result,
            ..
        } = &mut changed.functions[0].blocks[0].nodes[1].operation
        else {
            unreachable!()
        };
        match mutation {
            0 => fields.clear(),
            1 => fields[0].field = id(899, semantic_vocabulary::StructuralFieldId::new),
            2 => {
                fields[0].value = terminal_psi::RecordFieldValue::Scalar {
                    value: id(898, ValueId::new),
                    range_obligation: None,
                }
            }
            3 => *result_case = id(897, semantic_vocabulary::StructuralCaseId::new),
            4 => result.multiplicity = terminal_psi::StructuralMultiplicity::Linear,
            _ => unreachable!(),
        }
        refresh_identity(&mut changed);
        assert_ne!(original, changed.identity);
        assert!(
            !crate::unit_validation::operation_contracts::case_establishment_matches(
                &changed.functions[0],
                &changed.functions[0].blocks[0].nodes[1].operation,
                &types,
                &Default::default(),
            )
        );
    }
}

fn nested_case_unit(affine: bool) -> PsiOptimizationUnit {
    use crate::tests::fixtures::dominance::refresh_function_derivatives;
    use terminal_psi::{RecordFieldInitializer, RecordFieldValue, StructuralMultiplicity};

    let mut candidate = scalar_case_unit();
    let multiplicity = if affine {
        StructuralMultiplicity::Affine
    } else {
        StructuralMultiplicity::Unrestricted
    };
    let child_type = id(801, StructuralTypeId::new);
    let parent_type = id(820, StructuralTypeId::new);
    let parent_case = id(823, semantic_vocabulary::StructuralCaseId::new);
    let mut declarations = candidate.structural_types.to_vec();
    declarations.push(terminal_psi::StructuralTypeDeclaration {
        id: parent_type,
        identity: "test::NestedOutcome".into(),
        shape: terminal_psi::StructuralTypeShape::Sum {
            cases: vec![terminal_psi::StructuralCaseDeclaration {
                id: parent_case,
                identity: "Pair".into(),
                fields: [(821, "left"), (822, "right")]
                    .into_iter()
                    .map(|(raw, name)| terminal_psi::StructuralFieldDeclaration {
                        id: id(raw, semantic_vocabulary::StructuralFieldId::new),
                        identity: name.into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: terminal_psi::StructuralFieldType::Structural(child_type),
                    })
                    .collect(),
            }],
        },
    });
    candidate.structural_types = declarations.into();
    let child = &mut candidate.functions[0].blocks[0].nodes[1];
    let AbstractOperation::EstablishCase { result, .. } = &mut child.operation else {
        panic!("child case");
    };
    result.multiplicity = multiplicity;
    let mut second = child.clone();
    let AbstractOperation::EstablishCase {
        psi_operation,
        result,
        ..
    } = &mut second.operation
    else {
        unreachable!();
    };
    *psi_operation = id(815, OperationId::new);
    result.place = id(814, PlaceId::new);
    candidate.functions[0].blocks[0].nodes.insert(2, second);
    let mut parent = candidate.functions[0].blocks[0].nodes[1].clone();
    parent.operation = AbstractOperation::EstablishCase {
        psi_operation: id(825, OperationId::new),
        result: terminal_psi::StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: id(824, PlaceId::new),
            structural_type: parent_type,
            multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        result_case: parent_case,
        fields: [(821, 804), (822, 814)]
            .into_iter()
            .map(|(field, place)| RecordFieldInitializer {
                field: id(field, semantic_vocabulary::StructuralFieldId::new),
                value: RecordFieldValue::Structural(terminal_psi::StructuralArgument {
                    place: id(place, PlaceId::new),
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::Owned,
                }),
            })
            .collect(),
    };
    candidate.functions[0].blocks[0].nodes.insert(3, parent);
    for (place, operation, structural_type) in [(814, 815, child_type), (824, 825, parent_type)] {
        candidate.functions[0]
            .structural_places
            .push(terminal_psi::StructuralPlaceDeclaration {
                id: id(place, PlaceId::new),
                kind: StructuralPlaceKind::OperationResult {
                    producer: id(operation, OperationId::new),
                    structural_type,
                },
            });
    }
    candidate.functions[0].declared_places.extend(
        [804, 814, 824]
            .into_iter()
            .map(|place| id(place, PlaceId::new)),
    );
    if affine {
        let AbstractOperation::Return {
            cleanup_actions, ..
        } = &mut candidate.functions[0].blocks[0].nodes[4].operation
        else {
            panic!("scalar return");
        };
        cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(id(
            824,
            PlaceId::new,
        )));
    }
    refresh_function_derivatives(&mut candidate, 0);
    candidate
}

#[test]
fn nested_case_payloads_preserve_copy_and_affine_custody() {
    for affine in [false, true] {
        let candidate = nested_case_unit(affine);
        assert!(candidate.functions[0].blocks[0].nodes[3].uses.is_empty());
        assert_eq!(crate::validate_psi_optimization_unit(&candidate), Ok(()));
    }
}

#[test]
fn nested_case_payloads_reject_rewritten_sources_and_duplicate_affine_moves() {
    use crate::tests::fixtures::dominance::refresh_function_derivatives;
    use terminal_psi::{RecordFieldValue, StructuralAccess, StructuralMultiplicity};

    let original = nested_case_unit(true);
    assert_eq!(crate::validate_psi_optimization_unit(&original), Ok(()));
    for mutation in 0..7 {
        let mut changed = original.clone();
        if mutation == 0 {
            changed.functions[0].blocks[0].nodes.swap(1, 3);
        } else {
            let AbstractOperation::EstablishCase { result, fields, .. } =
                &mut changed.functions[0].blocks[0].nodes[3].operation
            else {
                panic!("parent case");
            };
            match mutation {
                1 => fields[1].value = fields[0].value.clone(),
                2 | 3 => {
                    let RecordFieldValue::Structural(argument) = &mut fields[0].value else {
                        unreachable!();
                    };
                    if mutation == 2 {
                        argument.access = StructuralAccess::SharedBorrow;
                    } else {
                        argument.place = id(899, PlaceId::new);
                    }
                }
                4 => result.multiplicity = StructuralMultiplicity::Unrestricted,
                5 => fields.swap(0, 1),
                6 => {
                    fields[0].value = RecordFieldValue::Scalar {
                        value: id(3, ValueId::new),
                        range_obligation: None,
                    }
                }
                _ => unreachable!(),
            }
        }
        refresh_function_derivatives(&mut changed, 0);
        assert!(
            crate::validate_psi_optimization_unit(&changed).is_err(),
            "mutation {mutation}"
        );
    }
}
