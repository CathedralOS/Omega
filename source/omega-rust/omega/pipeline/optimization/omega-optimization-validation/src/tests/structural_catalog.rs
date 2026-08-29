//! Structural type, domain, field, signature, and provider tests.

use super::*;

#[test]
fn structural_type_graph_accepts_dag_shared_descendants_and_disconnected_components() {
    let root = id(401, StructuralTypeId::new);
    let left = id(402, StructuralTypeId::new);
    let right = id(403, StructuralTypeId::new);
    let leaf = id(404, StructuralTypeId::new);
    let disconnected_sum = id(405, StructuralTypeId::new);
    let disconnected_leaf = id(406, StructuralTypeId::new);
    let mut candidate = unit();
    candidate.structural_types = vec![
        structural_type(
            401,
            psi_terminal::StructuralTypeShape::Mixed {
                fields: vec![structural_field(2, left)],
                cases: vec![structural_case(1, vec![structural_field(3, right)])],
            },
        ),
        structural_type(
            402,
            psi_terminal::StructuralTypeShape::Record {
                fields: vec![
                    structural_field(1, leaf),
                    structural_leaf_field(
                        5,
                        psi_terminal::BindingRelevance::Relevant,
                        psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
                    ),
                    structural_leaf_field(
                        6,
                        psi_terminal::BindingRelevance::Relevant,
                        psi_terminal::StructuralFieldType::IeeeFloat(
                            psi_core::IeeeFloatFormat::Binary32,
                        ),
                    ),
                    structural_leaf_field(
                        7,
                        psi_terminal::BindingRelevance::Relevant,
                        psi_terminal::StructuralFieldType::ByteSequence(
                            psi_terminal::ByteSequenceCarrier::BorrowedView,
                        ),
                    ),
                    structural_leaf_field(
                        8,
                        psi_terminal::BindingRelevance::Erased,
                        psi_terminal::StructuralFieldType::Erased {
                            type_identity: "validation::proof-only-leaf".into(),
                        },
                    ),
                ],
            },
        ),
        structural_type(
            403,
            psi_terminal::StructuralTypeShape::FixedArray {
                element: leaf,
                length: 2,
            },
        ),
        structural_type(
            404,
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        ),
        structural_type(
            405,
            psi_terminal::StructuralTypeShape::Sum {
                cases: vec![structural_case(
                    2,
                    vec![structural_field(4, disconnected_leaf)],
                )],
            },
        ),
        structural_type(
            406,
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        ),
    ];
    refresh_identity(&mut candidate);

    validate_psi_optimization_unit(&candidate)
        .expect("an acyclic catalog may share descendants and contain disconnected declarations");
    assert_eq!(
        candidate.structural_types[0].id, root,
        "the mixed root precedes every structural target it references"
    );
    assert_eq!(
        candidate.structural_types[4].id, disconnected_sum,
        "the disconnected sum precedes its structural target"
    );
}

#[test]
fn structural_type_graph_rejects_cycles_through_every_structural_edge_shape() {
    let recursive = id(410, StructuralTypeId::new);
    let shapes = vec![
        psi_terminal::StructuralTypeShape::Record {
            fields: vec![structural_field(10, recursive)],
        },
        psi_terminal::StructuralTypeShape::FixedArray {
            element: recursive,
            length: 1,
        },
        psi_terminal::StructuralTypeShape::Sum {
            cases: vec![structural_case(10, vec![structural_field(11, recursive)])],
        },
        psi_terminal::StructuralTypeShape::Mixed {
            fields: vec![structural_field(12, recursive)],
            cases: vec![structural_case(11, Vec::new())],
        },
        psi_terminal::StructuralTypeShape::Mixed {
            fields: Vec::new(),
            cases: vec![structural_case(12, vec![structural_field(13, recursive)])],
        },
    ];

    for shape in shapes {
        let mut candidate = unit();
        candidate.structural_types = vec![structural_type(410, shape)];
        refresh_identity(&mut candidate);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::RecursiveStructuralType(
                recursive
            ))
        );
    }
}

#[test]
fn structural_type_graph_rejects_an_unused_disconnected_cycle() {
    let first = id(420, StructuralTypeId::new);
    let second = id(421, StructuralTypeId::new);
    let mut candidate = unit();
    candidate.structural_types = vec![
        structural_type(
            419,
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        ),
        structural_type(
            420,
            psi_terminal::StructuralTypeShape::Record {
                fields: vec![structural_field(20, second)],
            },
        ),
        structural_type(
            421,
            psi_terminal::StructuralTypeShape::FixedArray {
                element: first,
                length: 1,
            },
        ),
    ];
    refresh_identity(&mut candidate);

    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::RecursiveStructuralType(
            first
        ))
    );
}

#[test]
fn structural_type_graph_reports_unknown_targets_before_recursion() {
    let recursive = id(430, StructuralTypeId::new);
    let unknown = id(431, StructuralTypeId::new);
    let mut candidate = unit();
    candidate.structural_types = vec![structural_type(
        430,
        psi_terminal::StructuralTypeShape::Record {
            fields: vec![
                structural_field(30, recursive),
                structural_field(31, unknown),
            ],
        },
    )];
    refresh_identity(&mut candidate);

    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::UnknownStructuralType(
            unknown
        ))
    );
}

#[test]
fn top_level_structural_type_roster_is_canonical_and_identity_unique() {
    let first = structural_type(
        450,
        psi_terminal::StructuralTypeShape::ByteSequence(
            psi_terminal::ByteSequenceCarrier::BorrowedView,
        ),
    );
    let second = structural_type(
        451,
        psi_terminal::StructuralTypeShape::ByteSequence(
            psi_terminal::ByteSequenceCarrier::BorrowedView,
        ),
    );

    let candidate = structural_catalog_unit(vec![second.clone(), first.clone()]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::NonCanonicalStructuralTypeOrder)
    );

    let candidate = structural_catalog_unit(vec![first.clone(), first.clone()]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::DuplicateStructuralType(
            first.id
        ))
    );

    let mut empty_identity = first.clone();
    empty_identity.identity.clear();
    let candidate = structural_catalog_unit(vec![empty_identity]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::InvalidStructuralTypeIdentity(first.id))
    );

    let mut duplicate_identity = second;
    duplicate_identity.identity = first.identity.clone();
    let candidate = structural_catalog_unit(vec![first, duplicate_identity.clone()]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::InvalidStructuralTypeIdentity(duplicate_identity.id))
    );
}

#[test]
fn top_level_structural_carriers_are_exact_without_narrowing_field_carriers() {
    let borrowed = id(460, StructuralTypeId::new);
    let array = id(461, StructuralTypeId::new);
    let record = id(462, StructuralTypeId::new);
    let candidate = structural_catalog_unit(vec![
        structural_type(
            460,
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        ),
        structural_type(
            461,
            psi_terminal::StructuralTypeShape::FixedArray {
                element: borrowed,
                length: 1,
            },
        ),
        structural_type(
            462,
            psi_terminal::StructuralTypeShape::Record {
                fields: vec![structural_leaf_field(
                    1,
                    psi_terminal::BindingRelevance::Relevant,
                    psi_terminal::StructuralFieldType::ByteSequence(
                        psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity: 8 },
                    ),
                )],
            },
        ),
    ]);
    validate_psi_optimization_unit(&candidate).expect(
        "BorrowedView and positive arrays are valid while field-level owned bytes stay legal",
    );
    assert_eq!(candidate.structural_types[2].id, record);

    for capacity in [0, 8] {
        let candidate = structural_catalog_unit(vec![structural_type(
            460,
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity },
            ),
        )]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::InvalidStructuralTypeIdentity(borrowed))
        );
    }

    let candidate = structural_catalog_unit(vec![
        structural_type(
            460,
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        ),
        structural_type(
            461,
            psi_terminal::StructuralTypeShape::FixedArray {
                element: borrowed,
                length: 0,
            },
        ),
    ]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::InvalidStructuralArrayLength(array))
    );
}

#[test]
fn structural_domain_roster_is_canonical_unique_and_carrier_closed() {
    let carrier = id(470, StructuralTypeId::new);
    let types = vec![structural_type(
        470,
        psi_terminal::StructuralTypeShape::ByteSequence(
            psi_terminal::ByteSequenceCarrier::BorrowedView,
        ),
    )];
    let first = structural_domain(1, 11, carrier);
    let second = structural_domain(2, 12, carrier);

    let mut candidate = structural_catalog_unit(types.clone());
    candidate.structural_domains = vec![first.clone(), second.clone()].into();
    refresh_identity(&mut candidate);
    validate_psi_optimization_unit(&candidate)
        .expect("distinct canonical domains may share one exact carrier");

    let mut candidate = structural_catalog_unit(types.clone());
    candidate.structural_domains = vec![second.clone(), first.clone()].into();
    refresh_identity(&mut candidate);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::NonCanonicalStructuralDomainOrder)
    );

    let mut candidate = structural_catalog_unit(types.clone());
    candidate.structural_domains = vec![first.clone(), first.clone()].into();
    refresh_identity(&mut candidate);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::DuplicateStructuralDomain(
            first.id
        ))
    );

    let mut invalid_identities = Vec::new();
    let mut empty_identity = first.clone();
    empty_identity.identity.clear();
    invalid_identities.push((vec![empty_identity], first.id));
    let mut duplicate_name = second.clone();
    duplicate_name.identity = first.identity.clone();
    invalid_identities.push((vec![first.clone(), duplicate_name], second.id));
    let mut duplicate_semantic = second.clone();
    duplicate_semantic.semantic_domain = first.semantic_domain;
    invalid_identities.push((vec![first.clone(), duplicate_semantic], second.id));
    for (domains, expected) in invalid_identities {
        let mut candidate = structural_catalog_unit(types.clone());
        candidate.structural_domains = domains.into();
        refresh_identity(&mut candidate);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::InvalidStructuralDomainIdentity(expected))
        );
    }

    let unknown = id(471, StructuralTypeId::new);
    let mut candidate = structural_catalog_unit(types);
    candidate.structural_domains = vec![structural_domain(1, 11, unknown)].into();
    refresh_identity(&mut candidate);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::UnknownStructuralType(
            unknown
        ))
    );
}

#[test]
fn structural_domain_content_projection_replays_terminal_contract() {
    let carrier = id(480, StructuralTypeId::new);
    let nested = id(481, StructuralTypeId::new);
    let non_record = id(482, StructuralTypeId::new);
    let types = vec![
        structural_type(
            480,
            psi_terminal::StructuralTypeShape::Record {
                fields: vec![
                    structural_leaf_field(
                        1,
                        psi_terminal::BindingRelevance::Relevant,
                        psi_terminal::StructuralFieldType::Structural(nested),
                    ),
                    structural_leaf_field(
                        2,
                        psi_terminal::BindingRelevance::Relevant,
                        psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
                    ),
                    structural_leaf_field(
                        3,
                        psi_terminal::BindingRelevance::Relevant,
                        psi_terminal::StructuralFieldType::Structural(non_record),
                    ),
                    structural_leaf_field(
                        4,
                        psi_terminal::BindingRelevance::Relevant,
                        psi_terminal::StructuralFieldType::IeeeFloat(
                            psi_core::IeeeFloatFormat::Binary32,
                        ),
                    ),
                    structural_leaf_field(
                        5,
                        psi_terminal::BindingRelevance::Relevant,
                        psi_terminal::StructuralFieldType::ByteSequence(
                            psi_terminal::ByteSequenceCarrier::BorrowedView,
                        ),
                    ),
                    structural_leaf_field(
                        6,
                        psi_terminal::BindingRelevance::Erased,
                        psi_terminal::StructuralFieldType::Erased {
                            type_identity: "validation::erased".into(),
                        },
                    ),
                ],
            },
        ),
        structural_type(
            481,
            psi_terminal::StructuralTypeShape::Record {
                fields: vec![structural_leaf_field(
                    1,
                    psi_terminal::BindingRelevance::Relevant,
                    psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
                )],
            },
        ),
        structural_type(
            482,
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        ),
    ];
    let domain_id = id(1, StructuralDomainId::new);
    let semantic_domain = id(31, psi_core::DomainSemanticId::new);
    let projection = |kind, parameter: &str, expression| {
        let algebra = psi_core::ContentAlgebra {
            kind,
            parameter: parameter.into(),
        };
        psi_terminal::StructuralContentProjection {
            identity: psi_core::ContentProjectionIdentity {
                domain: id(semantic_domain.get(), psi_core::ContentDomainId::new),
                projection_report_fingerprint:
                    psi_language_semantics::content::terminal_projection_report_fingerprint(
                        &algebra,
                        &expression,
                    ),
            },
            algebra,
            expression,
        }
    };
    let candidate_with = |projection| {
        let mut candidate = structural_catalog_unit(types.clone());
        let mut domain = structural_domain(1, semantic_domain.get(), carrier);
        domain.content_projection = Some(projection);
        candidate.structural_domains = vec![domain].into();
        refresh_identity(&mut candidate);
        candidate
    };
    let rejects = |projection| {
        assert_eq!(
            validate_psi_optimization_unit(&candidate_with(projection)),
            Err(
                OptimizationUnitValidationError::InvalidStructuralDomainContentProjection(
                    domain_id,
                ),
            )
        );
    };

    let nested_path = vec!["validation::field-1".into(), "validation::field-1".into()];
    let expression = ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Add(
        Box::new(ContentProjectionScalar::SubjectField(nested_path.clone())),
        Box::new(ContentProjectionScalar::Multiply(
            Box::new(ContentProjectionScalar::RuntimeScalarEmbedding(vec![
                "validation::field-2".into(),
            ])),
            Box::new(ContentProjectionScalar::Subtract(
                Box::new(ContentProjectionScalar::Successor(Box::new(
                    ContentProjectionScalar::Natural("0".into()),
                ))),
                Box::new(ContentProjectionScalar::Natural("1".into())),
            )),
        )),
    ));
    validate_psi_optimization_unit(&candidate_with(projection(
        psi_core::ContentAlgebraKind::CountedQuantity,
        "validation::unit",
        expression.clone(),
    )))
    .expect("the complete closed scalar grammar and nested Record paths are valid");

    for members in [
        Vec::new(),
        vec![(
            ContentProjectionScalar::SubjectField(nested_path),
            ContentProjectionScalar::Natural("9".into()),
        )],
    ] {
        validate_psi_optimization_unit(&candidate_with(projection(
            psi_core::ContentAlgebraKind::IntervalSet,
            "validation::coordinate-space",
            ContentProjectionExpression::IntervalSet(members),
        )))
        .expect("Terminal permits empty and symbolic interval sets");
    }

    let valid = projection(
        psi_core::ContentAlgebraKind::CountedQuantity,
        "validation::unit",
        expression,
    );
    let mut invalid = valid.clone();
    invalid.identity.domain = id(32, psi_core::ContentDomainId::new);
    rejects(invalid);
    let mut invalid = valid.clone();
    invalid.identity.projection_report_fingerprint = 0;
    rejects(invalid);
    let mut invalid = valid.clone();
    invalid.algebra.parameter.clear();
    rejects(invalid);
    let mut invalid = valid.clone();
    invalid.algebra.kind = psi_core::ContentAlgebraKind::IntervalSet;
    rejects(invalid);
    let invalid = projection(
        psi_core::ContentAlgebraKind::CountedQuantity,
        "validation::unit",
        ContentProjectionExpression::IntervalSet(Vec::new()),
    );
    rejects(invalid);
    let mut invalid = valid.clone();
    invalid.expression =
        ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Natural("2".into()));
    rejects(invalid);

    for value in ["", "00", "01", "1x", "١"] {
        rejects(projection(
            psi_core::ContentAlgebraKind::CountedQuantity,
            "validation::unit",
            ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Natural(
                value.into(),
            )),
        ));
    }
    for path in [
        Vec::new(),
        vec![String::new()],
        vec!["validation::missing".into()],
        vec!["validation::field-2".into(), "validation::field-1".into()],
        vec!["validation::field-1".into()],
        vec!["validation::field-3".into(), "validation::field-1".into()],
        vec!["validation::field-4".into()],
        vec!["validation::field-5".into()],
        vec!["validation::field-6".into()],
    ] {
        rejects(projection(
            psi_core::ContentAlgebraKind::CountedQuantity,
            "validation::unit",
            ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::SubjectField(
                path,
            )),
        ));
    }

    let nested_successors = |depth| {
        let mut scalar = ContentProjectionScalar::Natural("0".into());
        for _ in 0..depth {
            scalar = ContentProjectionScalar::Successor(Box::new(scalar));
        }
        scalar
    };
    validate_psi_optimization_unit(&candidate_with(projection(
        psi_core::ContentAlgebraKind::CountedQuantity,
        "validation::unit",
        ContentProjectionExpression::CountedQuantity(nested_successors(256)),
    )))
    .expect("Terminal's inclusive depth-256 boundary remains valid");
    rejects(projection(
        psi_core::ContentAlgebraKind::CountedQuantity,
        "validation::unit",
        ContentProjectionExpression::CountedQuantity(nested_successors(257)),
    ));
}

#[test]
fn structural_field_namespaces_require_canonical_ids_and_unique_nonempty_identities() {
    let owner = id(440, StructuralTypeId::new);
    let scalar_field = |raw| {
        structural_leaf_field(
            raw,
            psi_terminal::BindingRelevance::Relevant,
            psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
        )
    };

    let mut descending = vec![scalar_field(2), scalar_field(1)];
    let candidate = structural_catalog_unit(vec![structural_type(
        440,
        psi_terminal::StructuralTypeShape::Record {
            fields: descending.clone(),
        },
    )]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(
            OptimizationUnitValidationError::NonCanonicalStructuralFieldOrder {
                structural_type: owner,
                case: None,
            }
        )
    );

    descending.reverse();
    descending[1].identity = descending[0].identity.clone();
    let duplicate_name = descending[1].id;
    let candidate = structural_catalog_unit(vec![structural_type(
        440,
        psi_terminal::StructuralTypeShape::Mixed {
            fields: descending,
            cases: vec![structural_case(1, Vec::new())],
        },
    )]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(
            OptimizationUnitValidationError::InvalidStructuralFieldIdentity {
                structural_type: owner,
                field: duplicate_name,
            }
        )
    );

    let mut empty_name = scalar_field(1);
    empty_name.identity.clear();
    let empty_name_id = empty_name.id;
    let case = structural_case(1, vec![empty_name]);
    let case_id = case.id;
    let candidate = structural_catalog_unit(vec![structural_type(
        440,
        psi_terminal::StructuralTypeShape::Sum { cases: vec![case] },
    )]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(
            OptimizationUnitValidationError::InvalidStructuralFieldIdentity {
                structural_type: owner,
                field: empty_name_id,
            }
        )
    );

    let duplicate_id = vec![scalar_field(1), scalar_field(1)];
    let candidate = structural_catalog_unit(vec![structural_type(
        440,
        psi_terminal::StructuralTypeShape::Sum {
            cases: vec![structural_case(1, duplicate_id)],
        },
    )]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(
            OptimizationUnitValidationError::NonCanonicalStructuralFieldOrder {
                structural_type: owner,
                case: Some(case_id),
            }
        )
    );
}

#[test]
fn structural_cases_require_canonical_unique_nonempty_declarations() {
    let owner = id(441, StructuralTypeId::new);
    for shape in [
        psi_terminal::StructuralTypeShape::Sum { cases: Vec::new() },
        psi_terminal::StructuralTypeShape::Mixed {
            fields: Vec::new(),
            cases: Vec::new(),
        },
    ] {
        let candidate = structural_catalog_unit(vec![structural_type(441, shape)]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::EmptyStructuralSum(owner))
        );
    }

    let candidate = structural_catalog_unit(vec![structural_type(
        441,
        psi_terminal::StructuralTypeShape::Sum {
            cases: vec![
                structural_case(2, Vec::new()),
                structural_case(1, Vec::new()),
            ],
        },
    )]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::NonCanonicalStructuralCaseOrder(owner))
    );

    let first = structural_case(1, Vec::new());
    let mut duplicate = structural_case(2, Vec::new());
    duplicate.identity = first.identity.clone();
    let duplicate_id = duplicate.id;
    let candidate = structural_catalog_unit(vec![structural_type(
        441,
        psi_terminal::StructuralTypeShape::Mixed {
            fields: Vec::new(),
            cases: vec![first, duplicate],
        },
    )]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(
            OptimizationUnitValidationError::InvalidStructuralCaseIdentity {
                structural_type: owner,
                case: duplicate_id,
            }
        )
    );

    let mut empty = structural_case(1, Vec::new());
    empty.identity.clear();
    let empty_id = empty.id;
    let candidate = structural_catalog_unit(vec![structural_type(
        441,
        psi_terminal::StructuralTypeShape::Sum { cases: vec![empty] },
    )]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(
            OptimizationUnitValidationError::InvalidStructuralCaseIdentity {
                structural_type: owner,
                case: empty_id,
            }
        )
    );
}

#[test]
fn structural_field_namespaces_are_independent_and_payloadless_cases_are_valid() {
    let shared_field = || {
        structural_leaf_field(
            1,
            psi_terminal::BindingRelevance::Relevant,
            psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
        )
    };
    let candidate = structural_catalog_unit(vec![
        structural_type(
            442,
            psi_terminal::StructuralTypeShape::Mixed {
                fields: vec![shared_field()],
                cases: vec![
                    structural_case(1, vec![shared_field()]),
                    structural_case(2, vec![shared_field()]),
                    structural_case(3, Vec::new()),
                ],
            },
        ),
        structural_type(
            448,
            psi_terminal::StructuralTypeShape::Sum {
                cases: vec![structural_case(1, Vec::new())],
            },
        ),
    ]);
    validate_psi_optimization_unit(&candidate)
        .expect("field namespaces are independent and Sum/Mixed cases may be payloadless");
}

#[test]
fn structural_field_erasure_matrix_matches_canonical_terminal_admission() {
    let owner = id(443, StructuralTypeId::new);
    let invalid = vec![
        psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
        psi_terminal::StructuralFieldType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32),
        psi_terminal::StructuralFieldType::Structural(owner),
    ];
    for field_type in invalid {
        let field = structural_leaf_field(1, psi_terminal::BindingRelevance::Erased, field_type);
        let field_id = field.id;
        let candidate = structural_catalog_unit(vec![structural_type(
            443,
            psi_terminal::StructuralTypeShape::Record {
                fields: vec![field],
            },
        )]);
        assert_eq!(
            validate_psi_optimization_unit(&candidate),
            Err(
                OptimizationUnitValidationError::InvalidErasedStructuralField {
                    structural_type: owner,
                    field: field_id,
                }
            )
        );
    }

    for (raw, field_type) in [
        (
            1,
            psi_terminal::StructuralFieldType::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ),
        ),
        (
            2,
            psi_terminal::StructuralFieldType::Erased {
                type_identity: "validation::proof-only".into(),
            },
        ),
    ] {
        let candidate = structural_catalog_unit(vec![structural_type(
            443,
            psi_terminal::StructuralTypeShape::Record {
                fields: vec![structural_leaf_field(
                    raw,
                    psi_terminal::BindingRelevance::Erased,
                    field_type,
                )],
            },
        )]);
        validate_psi_optimization_unit(&candidate)
            .expect("Terminal admits the exact proof-side leaf carrier");
    }

    let empty_erased = structural_leaf_field(
        1,
        psi_terminal::BindingRelevance::Erased,
        psi_terminal::StructuralFieldType::Erased {
            type_identity: String::new(),
        },
    );
    let empty_erased_id = empty_erased.id;
    let candidate = structural_catalog_unit(vec![structural_type(
        443,
        psi_terminal::StructuralTypeShape::Sum {
            cases: vec![structural_case(1, vec![empty_erased])],
        },
    )]);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(
            OptimizationUnitValidationError::InvalidErasedStructuralField {
                structural_type: owner,
                field: empty_erased_id,
            }
        )
    );
}

#[test]
fn relevant_erased_field_requires_an_exact_record_provider_attachment_witness() {
    let owner = id(444, StructuralTypeId::new);
    let field = id(1, psi_core::StructuralFieldId::new);
    let provider_field = || {
        structural_leaf_field(
            1,
            psi_terminal::BindingRelevance::Relevant,
            psi_terminal::StructuralFieldType::Erased {
                type_identity: "validation::provider".into(),
            },
        )
    };
    let provider_place = |attachment, provider_field| psi_terminal::StructuralPlaceDeclaration {
        id: id(445, PlaceId::new),
        kind: StructuralPlaceKind::ProviderAttachment {
            attachment,
            field: provider_field,
            boundary: id(446, BoundaryMachineId::new),
        },
    };

    let valid = provider_attachment_specialization_unit();
    validate_psi_optimization_unit(&valid)
        .expect("a complete provider specialization witnesses its relevant erased field");

    for (attachment, provider_field_id) in [
        (None, None),
        (Some(owner), Some(id(2, psi_core::StructuralFieldId::new))),
        (Some(id(447, StructuralTypeId::new)), Some(field)),
    ] {
        let mut invalid = structural_catalog_unit(vec![structural_type(
            444,
            psi_terminal::StructuralTypeShape::Record {
                fields: vec![provider_field()],
            },
        )]);
        if let (Some(attachment), Some(provider_field_id)) = (attachment, provider_field_id) {
            invalid.functions[0].attachment = Some(attachment);
            invalid.functions[0]
                .structural_places
                .push(provider_place(attachment, provider_field_id));
        }
        refresh_identity(&mut invalid);
        assert_eq!(
            validate_psi_optimization_unit(&invalid),
            Err(
                OptimizationUnitValidationError::InvalidErasedStructuralField {
                    structural_type: owner,
                    field,
                }
            )
        );
    }

    for shape in [
        psi_terminal::StructuralTypeShape::Sum {
            cases: vec![structural_case(1, vec![provider_field()])],
        },
        psi_terminal::StructuralTypeShape::Mixed {
            fields: vec![provider_field()],
            cases: vec![structural_case(1, Vec::new())],
        },
        psi_terminal::StructuralTypeShape::Mixed {
            fields: Vec::new(),
            cases: vec![structural_case(1, vec![provider_field()])],
        },
    ] {
        let mut invalid = structural_catalog_unit(vec![structural_type(444, shape)]);
        invalid.functions[0].attachment = Some(owner);
        invalid.functions[0]
            .structural_places
            .push(provider_place(owner, field));
        refresh_identity(&mut invalid);
        assert_eq!(
            validate_psi_optimization_unit(&invalid),
            Err(
                OptimizationUnitValidationError::InvalidErasedStructuralField {
                    structural_type: owner,
                    field,
                }
            )
        );
    }
}

#[test]
fn structural_signatures_replay_attachment_and_unique_self_legality() {
    let mut attached = structural_call_unit();
    let structural_type = attached.structural_types[0].id;
    attached.functions[0].attachment = Some(structural_type);
    attached.functions[0].structural_parameters[0].is_self = true;
    let StructuralPlaceKind::Parameter { is_self, .. } =
        &mut attached.functions[0].structural_places[0].kind
    else {
        panic!("fixture retains its parameter root")
    };
    *is_self = true;
    refresh_identity(&mut attached);
    validate_psi_optimization_unit(&attached)
        .expect("one attachment-typed self parameter is canonical");

    let mut self_without_attachment = attached.clone();
    self_without_attachment.functions[0].attachment = None;
    refresh_identity(&mut self_without_attachment);
    assert!(matches!(
        validate_psi_optimization_unit(&self_without_attachment),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
    ));

    let mut mismatched_self = attached.clone();
    let alternate = id(4_710, StructuralTypeId::new);
    mismatched_self
        .structural_types
        .push(psi_terminal::StructuralTypeDeclaration {
            id: alternate,
            identity: "validation::alternate-attachment".into(),
            shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
        });
    mismatched_self.functions[0].attachment = Some(alternate);
    refresh_identity(&mut mismatched_self);
    assert!(matches!(
        validate_psi_optimization_unit(&mismatched_self),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
    ));

    let mut duplicate_self = attached.clone();
    let mut second = duplicate_self.functions[0].structural_parameters[0].clone();
    second.place = id(4_711, PlaceId::new);
    second.position = 1;
    duplicate_self.functions[0]
        .structural_parameters
        .push(second.clone());
    duplicate_self.functions[0]
        .structural_places
        .push(psi_terminal::StructuralPlaceDeclaration {
            id: second.place,
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: true,
            },
        });
    refresh_identity(&mut duplicate_self);
    assert!(matches!(
        validate_psi_optimization_unit(&duplicate_self),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
    ));

    let mut unknown_function_attachment = structural_call_unit();
    unknown_function_attachment.functions[0].attachment = Some(id(4_799, StructuralTypeId::new));
    refresh_identity(&mut unknown_function_attachment);
    assert!(matches!(
        validate_psi_optimization_unit(&unknown_function_attachment),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
    ));

    let mut boundary_self = byte_literal_boundary_unit();
    let boundary_type = boundary_self.boundary_machines[0].structural_parameters[0].structural_type;
    boundary_self.boundary_machines[0].attachment = Some(boundary_type);
    boundary_self.boundary_machines[0].structural_parameters[0].is_self = true;
    refresh_identity(&mut boundary_self);
    validate_psi_optimization_unit(&boundary_self)
        .expect("boundary self uses the exact known attachment type");

    boundary_self.boundary_machines[0].attachment = Some(id(4_798, StructuralTypeId::new));
    refresh_identity(&mut boundary_self);
    assert_eq!(
        validate_psi_optimization_unit(&boundary_self),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: None })
    );
}

#[test]
fn logical_structural_roots_are_unique_beyond_place_identity() {
    let mut duplicate = structural_result_call_unit();
    let first_call = duplicate.functions[0].blocks[0].nodes[0].clone();
    let (psi_operation, result_type) = match &first_call.operation {
        O::CallStructural {
            psi_operation,
            result,
            ..
        } => (*psi_operation, result.structural_type),
        _ => panic!("fixture begins with one structural call"),
    };
    let duplicate_place = id(4_712, PlaceId::new);
    let mut duplicate_call = first_call;
    let O::CallStructural {
        result: duplicate_result,
        ..
    } = &mut duplicate_call.operation
    else {
        unreachable!()
    };
    duplicate_result.place = duplicate_place;
    duplicate.functions[0].blocks[0]
        .nodes
        .insert(1, duplicate_call);
    duplicate.functions[0]
        .structural_places
        .push(psi_terminal::StructuralPlaceDeclaration {
            id: duplicate_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: psi_operation,
                structural_type: result_type,
            },
        });
    refresh_function_derivatives(&mut duplicate, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&duplicate),
        Err(
            OptimizationUnitValidationError::DuplicateStructuralPlaceRoot {
                machine: _,
                kind: StructuralPlaceKind::OperationResult { .. },
            }
        )
    ));
}

#[test]
fn boolean_structural_field_replays_terminal_root_and_cleanup_contract() {
    let baseline = boolean_structural_field_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("exact affine readable Boolean observation validates");
    let invalid = |mut candidate: PsiOptimizationUnit| {
        refresh_identity(&mut candidate);
        assert!(matches!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::InvalidBooleanStructuralField { .. })
        ));
    };

    let mut non_entry = baseline.clone();
    non_entry.entry = id(4_799, MachineId::new);
    invalid(non_entry);

    let mut unrestricted = baseline.clone();
    unrestricted.functions[0].structural_parameters[0].multiplicity =
        psi_terminal::StructuralMultiplicity::Unrestricted;
    invalid(unrestricted);

    let mut write_only = baseline.clone();
    write_only.functions[0].structural_parameters[0].access =
        psi_terminal::StructuralAccess::WriteOnlyBorrow;
    invalid(write_only);

    let mut qualified = baseline.clone();
    let domain = id(1, StructuralDomainId::new);
    qualified.structural_domains =
        vec![structural_domain(1, 1, qualified.structural_types[0].id)].into();
    qualified.functions[0].structural_parameters[0]
        .qualifications
        .push(domain);
    invalid(qualified);

    let mut claimed = baseline.clone();
    let claim = id(1, ClaimId::new);
    let source = claimed.functions[0].structural_parameters[0].place;
    claimed.functions[0]
        .entry_claim_declarations
        .push(psi_terminal::EntryClaim {
            claim,
            input: source,
            path: Vec::new(),
        });
    claimed.functions[0].entry_claims.insert(claim);
    invalid(claimed);

    let mut content_claimed = baseline.clone();
    install_content_owner(&mut content_claimed);
    content_claimed.functions[0]
        .content_entry_claims
        .push(content_entry_claim(claim, source));
    invalid(content_claimed);

    let mut no_boolean_parameter = baseline.clone();
    no_boolean_parameter.functions[0].parameters.clear();
    invalid(no_boolean_parameter);

    let mut missing_cleanup = baseline.clone();
    let O::Return {
        cleanup_actions, ..
    } = &mut missing_cleanup.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("fixture ends in a scalar return")
    };
    cleanup_actions.clear();
    refresh_node_derivatives(&mut missing_cleanup, 0, 0, 1);
    invalid(missing_cleanup);

    let mut wrong_field = baseline.clone();
    let O::BooleanStructuralField { field, .. } =
        &mut wrong_field.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with its observation")
    };
    *field = id(4_799, psi_core::StructuralFieldId::new);
    refresh_node_derivatives(&mut wrong_field, 0, 0, 0);
    invalid(wrong_field);

    let mut non_boolean_field = baseline.clone();
    let psi_terminal::StructuralTypeShape::Record { fields } =
        &mut non_boolean_field.structural_types[0].shape
    else {
        unreachable!()
    };
    fields[0].field_type = psi_terminal::StructuralFieldType::Scalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
    ));
    invalid(non_boolean_field);

    let mut differing_observation = baseline;
    let mut second = differing_observation.functions[0].blocks[0].nodes[0].clone();
    let second_field = id(4_713, psi_core::StructuralFieldId::new);
    let O::BooleanStructuralField {
        psi_operation,
        result,
        field,
        ..
    } = &mut second.operation
    else {
        unreachable!()
    };
    *psi_operation = id(4_714, OperationId::new);
    *result = id(4_715, ValueId::new);
    *field = second_field;
    let psi_terminal::StructuralTypeShape::Record { fields } =
        &mut differing_observation.structural_types[0].shape
    else {
        unreachable!()
    };
    fields.push(psi_terminal::StructuralFieldDeclaration {
        id: second_field,
        identity: "validation::other-ready".into(),
        relevance: psi_terminal::BindingRelevance::Relevant,
        field_type: psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean),
    });
    differing_observation.functions[0].blocks[0]
        .nodes
        .insert(1, second);
    refresh_function_derivatives(&mut differing_observation, 0);
    invalid(differing_observation);
}

#[test]
fn structural_returns_reject_non_source_roots_and_signature_drift() {
    let mut result_root = structural_result_call_unit();
    let result_place = result_root.functions[1]
        .result
        .structural()
        .expect("structural result")
        .place;
    let return_node = result_root.functions[1].blocks[0].nodes.len() - 1;
    let O::ReturnStructural { source, .. } =
        &mut result_root.functions[1].blocks[0].nodes[return_node].operation
    else {
        panic!("fixture returns structurally")
    };
    *source = result_place;
    refresh_node_derivatives(&mut result_root, 1, 0, return_node);
    assert!(matches!(
        validate_psi_optimization_unit(&result_root),
        Err(OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { .. })
    ));

    let mut literal_root = structural_result_call_unit();
    let literal_type = psi_terminal::StructuralTypeDeclaration {
        id: id(4_716, StructuralTypeId::new),
        identity: "validation::return-source-literal".into(),
        shape: psi_terminal::StructuralTypeShape::ByteSequence(
            psi_terminal::ByteSequenceCarrier::BorrowedView,
        ),
    };
    let literal = psi_terminal::StructuralPlaceDeclaration {
        id: id(4_717, PlaceId::new),
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: literal_type.id,
        },
    };
    literal_root.structural_types.push(literal_type.clone());
    literal_root.functions[1].structural_places.push(literal);
    let establishment_node = literal_root.functions[1].blocks[0].nodes[0].clone();
    literal_root.functions[1].blocks[0]
        .nodes
        .insert(0, establishment_node);
    literal_root.functions[1].blocks[0].nodes[0].operation = O::EstablishByteSequenceLiteral {
        psi_operation: id(4_718, OperationId::new),
        place: literal,
        structural_type: literal_type,
        bytes: b"return-source".to_vec(),
    };
    let O::ReturnStructural { source, .. } =
        &mut literal_root.functions[1].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    *source = literal.id;
    refresh_function_derivatives(&mut literal_root, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&literal_root),
        Err(OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { .. })
    ));

    let mut wrong_signature =
        operation_result_cfg_unit(OperationResultCfgShape::DominatingNonTopological);
    let O::CallStructural { result, .. } =
        &mut wrong_signature.functions[0].blocks[3].nodes[0].operation
    else {
        panic!("non-topological fixture stores its call in the entry block")
    };
    result.multiplicity = psi_terminal::StructuralMultiplicity::Affine;
    refresh_node_derivatives(&mut wrong_signature, 0, 3, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_signature),
        Err(OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { .. })
    ));
}

#[test]
fn provider_attachment_specialization_replays_exact_roots_calls_and_nonuse() {
    let baseline = provider_attachment_specialization_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("repeated calls share one canonical provider requirement root");
    let machine = baseline.functions[0].machine;
    let invalid = OptimizationUnitValidationError::InvalidProviderAttachmentSpecialization(machine);
    let attachment = baseline.functions[0]
        .attachment
        .expect("provider fixture attachment");
    let first_boundary = baseline.boundary_machines[0].id;
    let second_boundary = baseline.boundary_machines[1].id;
    let unused_boundary = baseline.boundary_machines[2].id;
    let first_provider_place = baseline.functions[0].structural_places[0].id;

    let assert_invalid = |mut unit: PsiOptimizationUnit| {
        refresh_identity(&mut unit);
        assert_eq!(validate_psi_optimization_unit(&unit), Err(invalid.clone()));
    };

    let mut missing_root = baseline.clone();
    missing_root.functions[0].structural_places.pop();
    assert_invalid(missing_root);

    let mut extra_root = baseline.clone();
    extra_root.functions[0]
        .structural_places
        .push(psi_terminal::StructuralPlaceDeclaration {
            id: id(453, PlaceId::new),
            kind: StructuralPlaceKind::ProviderAttachment {
                attachment,
                field: id(1, psi_core::StructuralFieldId::new),
                boundary: unused_boundary,
            },
        });
    assert_invalid(extra_root);

    let mut reordered_roots = baseline.clone();
    reordered_roots.functions[0].structural_places.swap(0, 1);
    assert_invalid(reordered_roots);

    let mut duplicate_root = baseline.clone();
    let duplicate_kind = duplicate_root.functions[0].structural_places[1].kind;
    duplicate_root.functions[0]
        .structural_places
        .push(psi_terminal::StructuralPlaceDeclaration {
            id: id(453, PlaceId::new),
            kind: duplicate_kind,
        });
    assert_invalid(duplicate_root);

    let mut wrong_field = baseline.clone();
    let StructuralPlaceKind::ProviderAttachment { field, .. } =
        &mut wrong_field.functions[0].structural_places[1].kind
    else {
        panic!("provider fixture root")
    };
    *field = id(2, psi_core::StructuralFieldId::new);
    assert_invalid(wrong_field);

    let mut unknown_boundary = baseline.clone();
    let StructuralPlaceKind::ProviderAttachment { boundary, .. } =
        &mut unknown_boundary.functions[0].structural_places[1].kind
    else {
        panic!("provider fixture root")
    };
    *boundary = id(999, BoundaryMachineId::new);
    assert_invalid(unknown_boundary);

    let mut attached_boundary = baseline.clone();
    attached_boundary.boundary_machines[1].attachment = Some(attachment);
    assert_invalid(attached_boundary);

    let mut self_parameter = baseline.clone();
    let parameter_place = id(454, PlaceId::new);
    self_parameter.functions[0].structural_parameters.push(
        psi_terminal::StructuralParameterDeclaration {
            place: parameter_place,
            position: 0,
            is_self: true,
            structural_type: attachment,
            multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
            access: psi_terminal::StructuralAccess::Owned,
            qualifications: Vec::new(),
        },
    );
    self_parameter.functions[0]
        .structural_places
        .push(psi_terminal::StructuralPlaceDeclaration {
            id: parameter_place,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: true,
            },
        });
    assert_invalid(self_parameter);

    let mut missing_call = baseline.clone();
    let AbstractOperation::BoundaryCall { boundary, .. } =
        &mut missing_call.functions[0].blocks[0].nodes[2].operation
    else {
        panic!("provider fixture call")
    };
    *boundary = first_boundary;
    assert_invalid(missing_call);

    let mut extra_call = baseline.clone();
    let AbstractOperation::BoundaryCall { boundary, .. } =
        &mut extra_call.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("provider fixture call")
    };
    *boundary = unused_boundary;
    assert_invalid(extra_call);

    let provider_argument = psi_terminal::StructuralArgument {
        place: first_provider_place,
        path: Vec::new(),
        access: psi_terminal::StructuralAccess::Owned,
    };
    let mut boundary_use = baseline.clone();
    let AbstractOperation::BoundaryCall {
        structural_arguments,
        ..
    } = &mut boundary_use.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("provider fixture call")
    };
    structural_arguments.push(provider_argument.clone());
    assert_invalid(boundary_use);

    let mut unit_use = baseline.clone();
    let psi_operation = match unit_use.functions[0].blocks[0].nodes[0].operation {
        AbstractOperation::BoundaryCall { psi_operation, .. } => psi_operation,
        _ => panic!("provider fixture call"),
    };
    unit_use.functions[0].blocks[0].nodes[0].operation = AbstractOperation::CallUnit {
        psi_operation,
        callee: machine,
        structural_arguments: vec![provider_argument],
        claim_transfers: Vec::new(),
    };
    refresh_node_derivatives(&mut unit_use, 0, 0, 0);
    assert_invalid(unit_use);

    let mut multiple_fields = baseline;
    let psi_terminal::StructuralTypeShape::Record { fields } =
        &mut multiple_fields.structural_types[0].shape
    else {
        panic!("provider fixture attachment record")
    };
    fields.push(structural_leaf_field(
        2,
        psi_terminal::BindingRelevance::Relevant,
        psi_terminal::StructuralFieldType::Erased {
            type_identity: "validation::second-provider".into(),
        },
    ));
    multiple_fields.functions[0]
        .structural_places
        .push(psi_terminal::StructuralPlaceDeclaration {
            id: id(453, PlaceId::new),
            kind: StructuralPlaceKind::ProviderAttachment {
                attachment,
                field: id(2, psi_core::StructuralFieldId::new),
                boundary: unused_boundary,
            },
        });
    assert_invalid(multiple_fields);

    assert!(first_boundary < second_boundary);
}
