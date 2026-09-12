//! Current inputs, nested ownership and provenance survive optimizer rewrites.
use super::super::*;
use terminal_psi::{
    RecordFieldInitializer, RecordFieldValue, StructuralAccess, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralTypeShape,
};

fn record_unit(affine: bool) -> PsiOptimizationUnit {
    let mut candidate = unit();
    let child_type = id(901, StructuralTypeId::new);
    let parent_type = id(902, StructuralTypeId::new);
    let scalar_type = candidate.functions[0].blocks[0].nodes[0].definitions[0].scalar_type;
    let field = |raw, name: &str, field_type| StructuralFieldDeclaration {
        id: id(raw, semantic_vocabulary::StructuralFieldId::new),
        identity: name.into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type,
    };
    candidate.structural_types = vec![
        terminal_psi::StructuralTypeDeclaration {
            id: child_type,
            identity: "test::Pair".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(903, "left", StructuralFieldType::Scalar(scalar_type)),
                    field(904, "right", StructuralFieldType::Scalar(scalar_type)),
                ],
            },
        },
        terminal_psi::StructuralTypeDeclaration {
            id: parent_type,
            identity: "test::Envelope".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(905, "first", StructuralFieldType::Structural(child_type)),
                    field(906, "second", StructuralFieldType::Structural(child_type)),
                ],
            },
        },
    ]
    .into();
    let multiplicity = if affine {
        StructuralMultiplicity::Affine
    } else {
        StructuralMultiplicity::Unrestricted
    };
    for ordinal in 0..3 {
        let place = id(910 + ordinal, PlaceId::new);
        let operation = id(920 + ordinal, OperationId::new);
        let structural_type = if ordinal < 2 { child_type } else { parent_type };
        candidate.functions[0]
            .structural_places
            .push(terminal_psi::StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type,
                },
            });
        candidate.functions[0].declared_places.insert(place);
        let fields = if ordinal < 2 {
            [903, 904]
                .into_iter()
                .map(|raw| RecordFieldInitializer {
                    field: id(raw, semantic_vocabulary::StructuralFieldId::new),
                    value: RecordFieldValue::Scalar {
                        value: id(3, ValueId::new),
                        range_obligation: None,
                    },
                })
                .collect()
        } else {
            [(905, 910), (906, 911)]
                .into_iter()
                .map(|(raw, source)| RecordFieldInitializer {
                    field: id(raw, semantic_vocabulary::StructuralFieldId::new),
                    value: RecordFieldValue::Structural(terminal_psi::StructuralArgument {
                        place: id(source, PlaceId::new),
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    }),
                })
                .collect()
        };
        let mut node = candidate.functions[0].blocks[0].nodes[0].clone();
        node.operation = AbstractOperation::EstablishRecord {
            psi_operation: operation,
            result: terminal_psi::StructuralOperationResult {
                place,
                structural_type,
                multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            fields,
        };
        candidate.functions[0].blocks[0]
            .nodes
            .insert(1 + ordinal as usize, node);
    }
    if affine {
        let AbstractOperation::Return {
            cleanup_actions, ..
        } = &mut candidate.functions[0].blocks[0].nodes[4].operation
        else {
            panic!("scalar fixture return");
        };
        cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(id(
            912,
            PlaceId::new,
        )));
    }
    refresh_function_derivatives(&mut candidate, 0);
    candidate
}

#[test]
fn record_construction_rejoins_scalar_uses_and_nested_owned_frontier() {
    for affine in [false, true] {
        let candidate = record_unit(affine);
        assert_eq!(validate_psi_optimization_unit(&candidate), Ok(()));
        let uses = &candidate.functions[0].blocks[0].nodes[1].uses;
        assert_eq!(uses.len(), 2);
        assert_eq!(uses[0].value, uses[1].value);
    }
}

#[test]
fn record_field_home_and_producer_corruption_rejects_after_metadata_refresh() {
    let original = record_unit(true);
    assert_eq!(validate_psi_optimization_unit(&original), Ok(()));
    for mutation in 0..9 {
        let mut changed = original.clone();
        let AbstractOperation::EstablishRecord { result, fields, .. } =
            &mut changed.functions[0].blocks[0].nodes[3].operation
        else {
            panic!("parent record");
        };
        match mutation {
            0 => {
                fields.pop();
            }
            1 => fields.swap(0, 1),
            2 => fields[0].field = id(903, semantic_vocabulary::StructuralFieldId::new),
            3 => {
                let RecordFieldValue::Structural(argument) = &mut fields[1].value else {
                    panic!("child record");
                };
                argument.place = id(910, PlaceId::new);
            }
            4 => {
                let RecordFieldValue::Structural(argument) = &mut fields[0].value else {
                    panic!("child record");
                };
                argument.access = StructuralAccess::SharedBorrow;
            }
            5 => {
                let RecordFieldValue::Structural(argument) = &mut fields[0].value else {
                    panic!("child record");
                };
                argument.place = result.place;
            }
            6 => result.multiplicity = StructuralMultiplicity::Linear,
            7 => changed.functions[0].blocks[0].nodes.swap(1, 3),
            8 => changed.functions[0].blocks[0].nodes.swap(0, 1),
            _ => unreachable!(),
        }
        refresh_function_derivatives(&mut changed, 0);
        assert_ne!(original.identity, changed.identity);
        assert!(
            validate_psi_optimization_unit(&changed).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn unrestricted_record_child_can_initialize_distinct_fields_without_consumption() {
    let mut candidate = record_unit(false);
    let AbstractOperation::EstablishRecord { fields, .. } =
        &mut candidate.functions[0].blocks[0].nodes[3].operation
    else {
        panic!("parent record");
    };
    let RecordFieldValue::Structural(argument) = &mut fields[1].value else {
        panic!("child record");
    };
    argument.place = id(910, PlaceId::new);
    refresh_function_derivatives(&mut candidate, 0);
    assert_eq!(validate_psi_optimization_unit(&candidate), Ok(()));
}

#[test]
fn bounded_record_fields_require_exact_current_value_and_interval_authority() {
    use semantic_vocabulary::{BoundedIntegerType, ObligationId, Proposition, ScalarTerm};
    let mut original = record_unit(false);
    let ScalarType::Integer(integer) =
        original.functions[0].blocks[0].nodes[0].definitions[0].scalar_type
    else {
        panic!("integer fixture");
    };
    let bounds = BoundedIntegerType::new(
        integer,
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(10),
    )
    .unwrap();
    let mut declarations = original.structural_types.to_vec();
    let StructuralTypeShape::Record { fields } = &mut declarations[0].shape else {
        panic!("child record")
    };
    fields[0].field_type = StructuralFieldType::BoundedInteger(bounds);
    original.structural_types = declarations.into();
    let proposition = |value, maximum| {
        let value = ScalarTerm::value(id(value, ValueId::new), ScalarType::Integer(integer));
        let endpoint = |value| ScalarTerm::Integer {
            scalar_type: integer,
            value: IntegerValue::Unsigned(value),
        };
        let mut clauses = vec![
            Proposition::LessOrEqual(endpoint(0), value.clone()),
            Proposition::LessOrEqual(value, endpoint(maximum)),
        ];
        clauses.sort();
        terminal_codec::canonical_proposition_order_key(&Proposition::Conjunction(clauses)).unwrap()
    };
    let mut facts = Vec::new();
    for ordinal in 0..2 {
        let AbstractOperation::EstablishRecord {
            psi_operation,
            fields,
            ..
        } = &mut original.functions[0].blocks[0].nodes[ordinal + 1].operation
        else {
            panic!("child record")
        };
        let obligation = id(930 + ordinal as u64, ObligationId::new);
        let RecordFieldValue::Scalar {
            range_obligation, ..
        } = &mut fields[0].value
        else {
            panic!("scalar field")
        };
        *range_obligation = Some(obligation);
        facts.push((*psi_operation, obligation));
    }
    refresh_function_derivatives(&mut original, 0);
    let facts = facts
        .into_iter()
        .map(|(operation, obligation)| {
            optimization_unit::AcceptedObligationFact::new(
                original.psi,
                [42; 32],
                original.functions[0].machine,
                operation,
                obligation,
                proposition(3, 10),
            )
        })
        .collect();
    let original = optimization_unit::attach_accepted_obligation_facts(original, facts).unwrap();
    assert_eq!(validate_psi_optimization_unit(&original), Ok(()));
    for mutation in 0..4 {
        let mut changed = original.clone();
        let fact = &mut changed.accepted_obligation_facts[0];
        let changed_proposition = match mutation {
            0 => proposition(3, 11),
            1 => proposition(4, 10),
            _ => fact.proposition.clone(),
        };
        *fact = optimization_unit::AcceptedObligationFact::new(
            fact.psi,
            fact.proof_bundle_fingerprint,
            fact.machine,
            if mutation == 2 {
                id(921, OperationId::new)
            } else {
                fact.operation
            },
            if mutation == 3 {
                id(999, ObligationId::new)
            } else {
                fact.obligation
            },
            changed_proposition,
        );
        refresh_function_derivatives(&mut changed, 0);
        assert!(
            validate_psi_optimization_unit(&changed).is_err(),
            "forged canonical authority {mutation}"
        );
    }
}

mod calls;
