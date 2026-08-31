use super::super::id;
use omega_optimization_unit::PsiOptimizationUnit;
use psi_core::{ClaimId, PlaceId, StructuralDomainId, StructuralTypeId};

pub(crate) fn content_entry_claim(
    claim: ClaimId,
    root: PlaceId,
) -> psi_terminal::ContentEntryClaim {
    let algebra = psi_core::ContentAlgebra {
        kind: psi_core::ContentAlgebraKind::CountedQuantity,
        parameter: "validation::content-only-claim".into(),
    };
    let expression = psi_core::ContentProjectionExpression::CountedQuantity(
        psi_core::ContentProjectionScalar::Natural("1".into()),
    );
    psi_terminal::ContentEntryClaim {
        claim,
        input: psi_core::ContentStructuralPlace {
            version: psi_core::ContentPlaceVersion::Entry,
            root,
            segments: Vec::new(),
        },
        projections: vec![psi_terminal::ClaimContentProjection {
            projection: psi_core::ContentProjectionIdentity {
                domain: id(1, psi_core::ContentDomainId::new),
                projection_report_fingerprint:
                    psi_language_semantics::content::terminal_projection_report_fingerprint(
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
    let semantic_domain = id(1, psi_core::DomainSemanticId::new);
    let algebra = psi_core::ContentAlgebra {
        kind: psi_core::ContentAlgebraKind::CountedQuantity,
        parameter: "validation::content-only-claim".into(),
    };
    let expression = psi_core::ContentProjectionExpression::CountedQuantity(
        psi_core::ContentProjectionScalar::Natural("1".into()),
    );
    unit.structural_domains = vec![psi_terminal::StructuralDomainDeclaration {
        id: id(1, StructuralDomainId::new),
        semantic_domain,
        identity: "validation::content-only-domain".into(),
        carrier,
        content_projection: Some(psi_terminal::StructuralContentProjection {
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
        }),
    }]
    .into();
}

pub(crate) fn structural_domain(
    raw: u64,
    semantic_raw: u64,
    carrier: StructuralTypeId,
) -> psi_terminal::StructuralDomainDeclaration {
    psi_terminal::StructuralDomainDeclaration {
        id: id(raw, StructuralDomainId::new),
        semantic_domain: id(semantic_raw, psi_core::DomainSemanticId::new),
        identity: format!("validation::domain-{raw}"),
        carrier,
        content_projection: None,
    }
}
