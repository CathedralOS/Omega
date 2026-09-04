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
