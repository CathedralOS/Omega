//! Structural type graph, field, case, and erasure declaration tests.

use super::super::*;

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
