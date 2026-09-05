//! Structural-domain content-projection replay tests.

use super::super::*;

#[test]
fn structural_domain_content_projection_replays_terminal_contract() {
    let carrier = id(480, StructuralTypeId::new);
    let nested = id(481, StructuralTypeId::new);
    let non_record = id(482, StructuralTypeId::new);
    let types = vec![
        structural_type(
            480,
            terminal_psi::StructuralTypeShape::Record {
                fields: vec![
                    structural_leaf_field(
                        1,
                        terminal_psi::BindingRelevance::Relevant,
                        terminal_psi::StructuralFieldType::Structural(nested),
                    ),
                    structural_leaf_field(
                        2,
                        terminal_psi::BindingRelevance::Relevant,
                        terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean),
                    ),
                    structural_leaf_field(
                        3,
                        terminal_psi::BindingRelevance::Relevant,
                        terminal_psi::StructuralFieldType::Structural(non_record),
                    ),
                    structural_leaf_field(
                        4,
                        terminal_psi::BindingRelevance::Relevant,
                        terminal_psi::StructuralFieldType::IeeeFloat(
                            semantic_vocabulary::IeeeFloatFormat::Binary32,
                        ),
                    ),
                    structural_leaf_field(
                        5,
                        terminal_psi::BindingRelevance::Relevant,
                        terminal_psi::StructuralFieldType::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView,
                        ),
                    ),
                    structural_leaf_field(
                        6,
                        terminal_psi::BindingRelevance::Erased,
                        terminal_psi::StructuralFieldType::Erased {
                            type_identity: "validation::erased".into(),
                        },
                    ),
                ],
            },
        ),
        structural_type(
            481,
            terminal_psi::StructuralTypeShape::Record {
                fields: vec![structural_leaf_field(
                    1,
                    terminal_psi::BindingRelevance::Relevant,
                    terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean),
                )],
            },
        ),
        structural_type(
            482,
            terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView,
            ),
        ),
    ];
    let domain_id = id(1, StructuralDomainId::new);
    let semantic_domain = id(31, semantic_vocabulary::DomainSemanticId::new);
    let projection = |kind, parameter: &str, expression| {
        let algebra = semantic_vocabulary::ContentAlgebra {
            kind,
            parameter: parameter.into(),
        };
        terminal_psi::StructuralContentProjection {
            identity: semantic_vocabulary::ContentProjectionIdentity {
                domain: id(
                    semantic_domain.get(),
                    semantic_vocabulary::ContentDomainId::new,
                ),
                projection_report_fingerprint:
                    language_semantics::content::terminal_projection_report_fingerprint(
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
        semantic_vocabulary::ContentAlgebraKind::CountedQuantity,
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
            semantic_vocabulary::ContentAlgebraKind::IntervalSet,
            "validation::coordinate-space",
            ContentProjectionExpression::IntervalSet(members),
        )))
        .expect("Terminal permits empty and symbolic interval sets");
    }

    let valid = projection(
        semantic_vocabulary::ContentAlgebraKind::CountedQuantity,
        "validation::unit",
        expression,
    );
    let mut invalid = valid.clone();
    invalid.identity.domain = id(32, semantic_vocabulary::ContentDomainId::new);
    rejects(invalid);
    let mut invalid = valid.clone();
    invalid.identity.projection_report_fingerprint = 0;
    rejects(invalid);
    let mut invalid = valid.clone();
    invalid.algebra.parameter.clear();
    rejects(invalid);
    let mut invalid = valid.clone();
    invalid.algebra.kind = semantic_vocabulary::ContentAlgebraKind::IntervalSet;
    rejects(invalid);
    let invalid = projection(
        semantic_vocabulary::ContentAlgebraKind::CountedQuantity,
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
            semantic_vocabulary::ContentAlgebraKind::CountedQuantity,
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
            semantic_vocabulary::ContentAlgebraKind::CountedQuantity,
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
        semantic_vocabulary::ContentAlgebraKind::CountedQuantity,
        "validation::unit",
        ContentProjectionExpression::CountedQuantity(nested_successors(256)),
    )))
    .expect("Terminal's inclusive depth-256 boundary remains valid");
    rejects(projection(
        semantic_vocabulary::ContentAlgebraKind::CountedQuantity,
        "validation::unit",
        ContentProjectionExpression::CountedQuantity(nested_successors(257)),
    ));
}
