use super::super::{
    CheckedBooleanExpression, CheckedScalarExpression, ScalarBindings, StructuralMultiplicity,
    terminal_scalar_type,
};
use super::{
    PlaceId, ScalarType, StructuralAccess, StructuralFieldId, StructuralFieldType,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeId,
    StructuralTypeShape,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::emit_direct_expression;
use checked_trees::CheckedStructuralPredicatePathSegment;
use checked_trees::types::PrimitiveType;
use terminal_psi::OperationKind;
use terminal_psi::{BindingRelevance, StructuralFieldDeclaration};

#[test]
fn primitive_payload_reads_resolve_each_projection_and_reject_substitution() {
    use CheckedStructuralPredicatePathSegment::{Case, Field, FixedIndex};
    let scalar = terminal_scalar_type(PrimitiveType::I32).unwrap();
    let primitive = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let declarations = vec![
        StructuralTypeDeclaration {
            id: primitive,
            identity: "i32".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar),
        },
        StructuralTypeDeclaration {
            id: array,
            identity: "[i32;2]".into(),
            shape: StructuralTypeShape::FixedArray {
                element: primitive,
                length: 2,
            },
        },
    ];
    let bindings = payload_bindings(declarations, array);
    let path = vec![
        Case("Active".into()),
        Field("payload".into()),
        FixedIndex(1),
    ];
    let (source, remainder) = super::resolve_primitive(&bindings, 3, &path, scalar).unwrap();
    assert_eq!(
        remainder,
        vec![terminal_psi::StructuralPathSegment::FixedIndex(1)]
    );
    assert!(
        matches!(source, crate::emission::operation_emission::expressions::PrimitiveReadSource::CaseLeaf { structural_type, path, .. }
        if structural_type == array && path.len() == 2)
    );
    // Type resolution preserves a different declared case's identity; only
    // independent control-flow replay can decide whether that case is active.
    let mut other = path.clone();
    other[0] = Case("Other".into());
    assert_ne!(
        super::resolve_primitive(&bindings, 3, &path, scalar)
            .unwrap()
            .0,
        super::resolve_primitive(&bindings, 3, &other, scalar)
            .unwrap()
            .0
    );
    for replacement in [
        Case("Absent".into()),
        Field("missing".into()),
        FixedIndex(2),
    ] {
        let mut wrong = path.clone();
        let position = match &replacement {
            Case(_) => 0,
            Field(_) => 1,
            FixedIndex(_) => 2,
            CheckedStructuralPredicatePathSegment::RuntimeIndex(_) => 0,
        };
        wrong[position] = replacement;
        assert!(super::resolve_primitive(&bindings, 3, &wrong, scalar).is_err());
    }
    assert!(super::resolve_primitive(&bindings, 0, &path, scalar).is_err());
    assert!(super::resolve_primitive(&bindings, 3, &path, ScalarType::Boolean).is_err());
}

#[test]
fn whole_owned_array_read_uses_its_declared_carrier_not_a_borrowed_view() {
    let scalar_type = terminal_scalar_type(PrimitiveType::I32).unwrap();
    let element = StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).unwrap(),
        identity: "i32".to_owned(),
        shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
    };
    let array = StructuralTypeDeclaration {
        id: StructuralTypeId::new(2).unwrap(),
        identity: "[i32;3]".to_owned(),
        shape: StructuralTypeShape::FixedArray {
            element: element.id,
            length: 3,
        },
    };
    let parameter = StructuralParameterDeclaration {
        place: PlaceId::new(11).unwrap(),
        position: 0,
        is_self: false,
        structural_type: array.id,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let declarations = [element, array];
    let expression = CheckedScalarExpression::StructuralParameterIndexedRead {
        root: checked_trees::CheckedStorageRoot::Parameter { index: 3 },
        path: Vec::new(),
        inner_indexes: Vec::new(),
        element_path: Vec::new(),
        primitive_type: PrimitiveType::I32,
        index: Box::new(CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U64,
        }),
    };
    let bindings = ScalarBindings::new([terminal_scalar_type(PrimitiveType::U64).unwrap()])
        .with_structural_parameters(&[(3, parameter.clone())])
        .with_structural_observations(&declarations);
    assert!(matches!(bindings.expression(&expression).unwrap(),
        crate::emission::operation_emission::expressions::LoweredDirectExpression::IndexedPrimitiveRead { source, path, scalar_type: actual, .. }
        if source == parameter.place && path.is_empty() && actual == scalar_type));
    let mut wrong_type = expression.clone();
    let CheckedScalarExpression::StructuralParameterIndexedRead { primitive_type, .. } =
        &mut wrong_type
    else {
        unreachable!()
    };
    *primitive_type = PrimitiveType::U64;
    assert!(bindings.expression(&wrong_type).is_err());
    let mut unreadable = parameter;
    unreadable.access = StructuralAccess::WriteOnlyBorrow;
    assert!(
        ScalarBindings::new([terminal_scalar_type(PrimitiveType::U64).unwrap()])
            .with_structural_parameters(&[(3, unreadable)])
            .with_structural_observations(&declarations)
            .expression(&expression)
            .is_err()
    );
}

fn bindings(access: StructuralAccess, relevance: BindingRelevance) -> ScalarBindings {
    let scalar_type = terminal_scalar_type(PrimitiveType::U64).unwrap();
    let declaration = StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).unwrap(),
        identity: "Counter".to_owned(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: StructuralFieldId::new(7).unwrap(),
                identity: "value".to_owned(),
                relevance,
                field_type: StructuralFieldType::Scalar(scalar_type),
            }],
        },
    };
    let parameter = StructuralParameterDeclaration {
        place: PlaceId::new(11).unwrap(),
        position: 0,
        is_self: false,
        structural_type: declaration.id,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    ScalarBindings::new([])
        .with_structural_parameters(&[(3, parameter)])
        .with_structural_observations(&[declaration])
}

fn observation(
    position: u32,
    primitive_type: PrimitiveType,
    identity: &str,
) -> CheckedScalarExpression {
    CheckedScalarExpression::StructuralParameterField {
        parameter_position: position,
        path: vec![CheckedStructuralPredicatePathSegment::Field(
            identity.to_owned(),
        )],
        primitive_type,
    }
}

#[test]
fn field_reads_require_exact_source_position_type_name_and_readable_relevance() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let bindings = bindings(access, BindingRelevance::Relevant);
        assert!(
            bindings
                .expression(&observation(3, PrimitiveType::U64, "value"))
                .is_ok()
        );
        for expression in [
            observation(0, PrimitiveType::U64, "value"),
            observation(3, PrimitiveType::U8, "value"),
            observation(3, PrimitiveType::U64, "missing"),
        ] {
            assert!(bindings.expression(&expression).is_err());
        }
        let mut nested = observation(3, PrimitiveType::U64, "value");
        if let CheckedScalarExpression::StructuralParameterField { path, .. } = &mut nested {
            path.push(CheckedStructuralPredicatePathSegment::Field(
                "value".to_owned(),
            ));
        }
        assert!(bindings.expression(&nested).is_err());
        assert!(
            bindings
                .with_structural_parameters(&[])
                .expression(&observation(3, PrimitiveType::U64, "value"))
                .is_err()
        );
    }
    for (access, relevance) in [
        (
            StructuralAccess::WriteOnlyBorrow,
            BindingRelevance::Relevant,
        ),
        (StructuralAccess::MutableBorrow, BindingRelevance::Erased),
    ] {
        assert!(
            bindings(access, relevance)
                .expression(&observation(3, PrimitiveType::U64, "value"))
                .is_err()
        );
    }
}

fn nested_declarations(scalar: ScalarType) -> Vec<StructuralTypeDeclaration> {
    vec![
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(1).unwrap(),
            identity: "Root".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(7).unwrap(),
                    identity: "child".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(StructuralTypeId::new(2).unwrap()),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(2).unwrap(),
            identity: "Child".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(7).unwrap(),
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(scalar),
                }],
            },
        },
    ]
}

fn nested_observation(boolean: bool) -> CheckedScalarExpression {
    let path = ["child", "value"]
        .map(|identity| CheckedStructuralPredicatePathSegment::Field(identity.into()))
        .to_vec();
    if boolean {
        CheckedScalarExpression::Boolean(Box::new(
            CheckedBooleanExpression::StructuralParameterField {
                parameter_position: 3,
                path,
            },
        ))
    } else {
        CheckedScalarExpression::StructuralParameterField {
            parameter_position: 3,
            path,
            primitive_type: PrimitiveType::U64,
        }
    }
}

fn payload_bindings(
    mut declarations: Vec<StructuralTypeDeclaration>,
    member_type: StructuralTypeId,
) -> super::StructuralFieldBindings {
    let sum = StructuralTypeId::new(100).unwrap();
    declarations.push(StructuralTypeDeclaration {
        id: sum,
        identity: "Response".into(),
        shape: StructuralTypeShape::Sum {
            cases: ["Active", "Other"]
                .into_iter()
                .enumerate()
                .map(
                    |(ordinal, identity)| terminal_psi::StructuralCaseDeclaration {
                        id: semantic_vocabulary::StructuralCaseId::new(ordinal as u64 + 1).unwrap(),
                        identity: identity.into(),
                        fields: vec![StructuralFieldDeclaration {
                            id: StructuralFieldId::new(9).unwrap(),
                            identity: "payload".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(member_type),
                        }],
                    },
                )
                .collect(),
        },
    });
    super::StructuralScalarFieldBinding::collect(
        &[(
            3,
            StructuralParameterDeclaration {
                place: PlaceId::new(11).unwrap(),
                position: 0,
                is_self: false,
                structural_type: sum,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
        )],
        &declarations,
    )
}

fn payload_path(carriers: &[&str]) -> Vec<CheckedStructuralPredicatePathSegment> {
    use CheckedStructuralPredicatePathSegment::{Case, Field};
    let mut path = vec![Case("Active".into()), Field("payload".into())];
    path.extend(carriers.iter().map(|identity| Field((*identity).into())));
    path.push(Field("value".into()));
    path
}

#[test]
fn selected_payload_observations_preserve_full_paths_and_case_authority() {
    use super::CasePayloadRead;
    use crate::emission::operation_emission::expressions::PrimitiveReadSource;
    let scalar = terminal_scalar_type(PrimitiveType::U64).unwrap();
    let mut declarations = nested_declarations(scalar);
    let StructuralTypeShape::Record { fields } = &mut declarations[0].shape else {
        unreachable!()
    };
    let mut sibling = fields[0].clone();
    sibling.identity = "sibling".into();
    sibling.id = StructuralFieldId::new(8).unwrap();
    fields.push(sibling);
    let bindings = payload_bindings(declarations, StructuralTypeId::new(1).unwrap());
    let left = payload_path(&["child"]);
    let right = payload_path(&["sibling"]);
    let first = super::plan_case_payload_leaf(&bindings, 3, &left).unwrap();
    let second = super::plan_case_payload_leaf(&bindings, 3, &right).unwrap();
    assert_eq!(first.field, second.field);
    assert_ne!(first.path, second.path);
    // Declaration resolution describes the read; it grants no case authority.
    // Both paths prepare their own carrier at execution, where the emitted
    // copy still owes an independently verified active-case fact.
    for (path, planned) in [(&left, &first), (&right, &second)] {
        let CasePayloadRead::Observation {
            source,
            path: observed,
            field,
        } = super::resolve_case_payload(&bindings, 3, path, scalar).unwrap()
        else {
            panic!("nested payload read must retain its observation");
        };
        assert_eq!(observed, planned.path);
        assert_eq!(field, planned.field);
        assert!(matches!(source, PrimitiveReadSource::CaseLeaf {
            source, path, structural_type
        } if source == planned.source
            && path == vec![
                semantic_vocabulary::CanonicalStructuralPathSegment::Case(planned.case),
                semantic_vocabulary::CanonicalStructuralPathSegment::Field(planned.member),
            ]
            && structural_type == planned.member_type));
    }
    let mut wrong_case = left.clone();
    wrong_case[0] = CheckedStructuralPredicatePathSegment::Case("Other".into());
    assert!(matches!(
        super::resolve_case_payload(&bindings, 3, &wrong_case, scalar),
        Ok(CasePayloadRead::Observation {
            source: PrimitiveReadSource::CaseLeaf { path, .. }, ..
        }) if path[0] == semantic_vocabulary::CanonicalStructuralPathSegment::Case(
            semantic_vocabulary::StructuralCaseId::new(2).unwrap())
    ));
    wrong_case[0] = CheckedStructuralPredicatePathSegment::Case("Absent".into());
    assert!(super::resolve_case_payload(&bindings, 3, &wrong_case, scalar).is_err());
    assert!(super::resolve_case_payload(&bindings, 0, &left, scalar).is_err());
    assert!(super::resolve_case_payload(&bindings, 3, &left, ScalarType::Boolean).is_err());
    assert!(super::resolve_case_payload(&bindings, 3, &payload_path(&["absent"]), scalar).is_err());
}

#[test]
fn selected_payload_field_copy_rejects_a_well_typed_foreign_case_after_decode() {
    let checked = crate::front_end::checked_program(
        "pub data Value { number: i32; }
         pub data Reply { case First(value: Value); case Second(value: Value); }
         pub machine read(subject: &Reply) -> i32 {
             transition subject {
                 Reply::First { value } -> (value.number)
                 Reply::Second { value } -> (value.number)
             }
         }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("read"),
    )
    .produce(
        terminal_production::TerminalProductionCustody::artifact_only(
            &mut terminal_production::TerminalProductionTimings::default(),
        ),
    )
    .expect("both selected record payloads produce")
    .into_artifact();
    let mut module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile).unwrap();
    let cases = module
        .structural_types
        .iter()
        .find_map(|declaration| match &declaration.shape {
            StructuralTypeShape::Sum { cases } if cases.len() == 2 => Some([
                (cases[0].id, cases[0].fields[0].id),
                (cases[1].id, cases[1].fields[0].id),
            ]),
            _ => None,
        })
        .expect("reply case roster");
    let copy = module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            terminal_psi::OperationKind::StructuralCaseLeafCopy { path, .. } => Some(path),
            _ => None,
        })
        .expect("the read establishes its selected carrier");
    let semantic_vocabulary::CanonicalStructuralPathSegment::Case(selected) = &mut copy[0] else {
        panic!("copy retains case identity");
    };
    let replacement = if *selected == cases[0].0 {
        cases[1]
    } else {
        cases[0]
    };
    *selected = replacement.0;
    copy[1] = semantic_vocabulary::CanonicalStructuralPathSegment::Field(replacement.1);
    terminal_verifier::validate_module(&module)
        .expect("the other case has the same declared payload carrier");
    assert!(
        terminal_verifier::verify_module(&module, &proof, &profile).is_err(),
        "matching carrier layout cannot authorize the inactive case"
    );
}

#[test]
fn selected_payload_projection_uses_the_shared_carrier_walk_at_any_depth() {
    let scalar = terminal_scalar_type(PrimitiveType::U64).unwrap();
    for depth in [0, 1, 5, 12] {
        let mut declarations = nested_declarations(scalar);
        let mut member_type = StructuralTypeId::new(1).unwrap();
        for ordinal in 0..depth {
            let id = StructuralTypeId::new(ordinal + 3).unwrap();
            declarations.push(StructuralTypeDeclaration {
                id,
                identity: format!("Wrapper{ordinal}"),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(7).unwrap(),
                        identity: "next".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(member_type),
                    }],
                },
            });
            member_type = id;
        }
        let mut carriers = vec!["next"; depth as usize];
        carriers.push("child");
        let path = payload_path(&carriers);
        let bindings = payload_bindings(declarations.clone(), member_type);
        let planned = super::plan_case_payload_leaf(&bindings, 3, &path).unwrap();
        assert_eq!(planned.path.len(), depth as usize + 1);
        for mutation in ["erased", "duplicate", "missing"] {
            let mut changed = declarations.clone();
            match mutation {
                "duplicate" => changed.push(changed[1].clone()),
                "missing" => {
                    changed.remove(1);
                }
                _ => {
                    let StructuralTypeShape::Record { fields } = &mut changed[0].shape else {
                        unreachable!()
                    };
                    fields[0].relevance = BindingRelevance::Erased;
                }
            }
            assert!(
                super::plan_case_payload_leaf(&payload_bindings(changed, member_type), 3, &path)
                    .is_none(),
                "{mutation} at depth {depth}"
            );
        }
    }
}

#[test]
fn nested_scalar_reads_emit_original_root_and_declaration_local_carrier_ids() {
    for boolean in [false, true] {
        let scalar = if boolean {
            ScalarType::Boolean
        } else {
            terminal_scalar_type(PrimitiveType::U64).unwrap()
        };
        let declarations = nested_declarations(scalar);
        let bindings = bindings(StructuralAccess::MutableBorrow, BindingRelevance::Relevant)
            .with_structural_observations(&declarations);
        let expression = bindings.expression(&nested_observation(boolean)).unwrap();
        let mut operations = OperationBuffer::new(0);
        let mut next_value = 1;
        let first = emit_direct_expression(&expression, &[], &mut next_value, &mut operations);
        let second = emit_direct_expression(&expression, &[], &mut next_value, &mut operations);
        assert_ne!(first, second);
        assert_eq!(operations.len(), 2);
        for operation in operations.iter() {
            let (source, path, field) = match &operation.kind {
                OperationKind::IntegerStructuralField {
                    source,
                    path,
                    field,
                }
                | OperationKind::BooleanStructuralField {
                    source,
                    path,
                    field,
                } => (source, path, field),
                other => panic!("expected nested read, got {other:?}"),
            };
            assert_eq!(*source, PlaceId::new(11).unwrap());
            assert_eq!(*field, StructuralFieldId::new(7).unwrap());
            assert_eq!(
                path,
                &[semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    *field
                )]
            );
        }
        assert!(
            bindings
                .expression(&observation(3, PrimitiveType::U64, "value"))
                .is_err()
        );
        for mutation in [
            "erased carrier",
            "scalar carrier",
            "missing child",
            "duplicate type",
            "duplicate name",
            "duplicate id",
            "cycle",
            "wrong leaf",
        ] {
            let mut changed = declarations.clone();
            match mutation {
                "missing child" => {
                    changed.pop();
                }
                "duplicate type" => changed.push(changed[1].clone()),
                "wrong leaf" => {
                    let StructuralTypeShape::Record { fields } = &mut changed[1].shape else {
                        unreachable!()
                    };
                    fields[0].field_type = StructuralFieldType::Scalar(
                        terminal_scalar_type(PrimitiveType::U8).unwrap(),
                    );
                }
                _ => {
                    let StructuralTypeShape::Record { fields } = &mut changed[0].shape else {
                        unreachable!()
                    };
                    match mutation {
                        "erased carrier" => fields[0].relevance = BindingRelevance::Erased,
                        "scalar carrier" => {
                            fields[0].field_type = StructuralFieldType::Scalar(scalar)
                        }
                        "cycle" => {
                            fields[0].field_type =
                                StructuralFieldType::Structural(StructuralTypeId::new(1).unwrap())
                        }
                        "duplicate name" => {
                            let mut duplicate = fields[0].clone();
                            duplicate.id = StructuralFieldId::new(8).unwrap();
                            fields.push(duplicate);
                        }
                        "duplicate id" => {
                            let mut duplicate = fields[0].clone();
                            duplicate.identity = "other".into();
                            fields.push(duplicate);
                        }
                        _ => unreachable!(),
                    }
                }
            }
            let changed = bindings.clone().with_structural_observations(&changed);
            assert!(
                changed.expression(&nested_observation(boolean)).is_err(),
                "{boolean} {mutation}"
            );
        }
    }
}
