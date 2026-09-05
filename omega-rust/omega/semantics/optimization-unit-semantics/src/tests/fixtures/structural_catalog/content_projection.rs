use super::super::id;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{ClaimId, PlaceId, StructuralDomainId, StructuralTypeId};

pub(crate) fn content_entry_claim(
    claim: ClaimId,
    root: PlaceId,
) -> terminal_psi::ContentEntryClaim {
    let algebra = semantic_vocabulary::ContentAlgebra {
        kind: semantic_vocabulary::ContentAlgebraKind::CountedQuantity,
        parameter: "validation::content-only-claim".into(),
    };
    let expression = semantic_vocabulary::ContentProjectionExpression::CountedQuantity(
        semantic_vocabulary::ContentProjectionScalar::Natural("1".into()),
    );
    terminal_psi::ContentEntryClaim {
        claim,
        input: semantic_vocabulary::ContentStructuralPlace {
            version: semantic_vocabulary::ContentPlaceVersion::Entry,
            root,
            segments: Vec::new(),
        },
        projections: vec![terminal_psi::ClaimContentProjection {
            projection: semantic_vocabulary::ContentProjectionIdentity {
                domain: id(1, semantic_vocabulary::ContentDomainId::new),
                projection_report_fingerprint:
                    language_semantics::content::terminal_projection_report_fingerprint(
                        &algebra,
                        &expression,
                    ),
            },
            algebra,
        }],
    }
}

pub(crate) fn install_content_owner(unit: &mut PsiOptimizationUnit) {
    let carrier = unit.structural_types[0].id;
    let semantic_domain = id(1, semantic_vocabulary::DomainSemanticId::new);
    let algebra = semantic_vocabulary::ContentAlgebra {
        kind: semantic_vocabulary::ContentAlgebraKind::CountedQuantity,
        parameter: "validation::content-only-claim".into(),
    };
    let expression = semantic_vocabulary::ContentProjectionExpression::CountedQuantity(
        semantic_vocabulary::ContentProjectionScalar::Natural("1".into()),
    );
    unit.structural_domains = vec![terminal_psi::StructuralDomainDeclaration {
        id: id(1, StructuralDomainId::new),
        semantic_domain,
        identity: "validation::content-only-domain".into(),
        carrier,
        content_projection: Some(terminal_psi::StructuralContentProjection {
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
        }),
    }]
    .into();
}

pub(crate) fn structural_domain(
    raw: u64,
    semantic_raw: u64,
    carrier: StructuralTypeId,
) -> terminal_psi::StructuralDomainDeclaration {
    terminal_psi::StructuralDomainDeclaration {
        id: id(raw, StructuralDomainId::new),
        semantic_domain: id(semantic_raw, semantic_vocabulary::DomainSemanticId::new),
        identity: format!("validation::domain-{raw}"),
        carrier,
        content_projection: None,
    }
}
