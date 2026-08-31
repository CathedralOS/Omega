//! Content-only internal claim-transfer admission and interface corruption rejection.

use crate::tests::{
    content_entry_claim, id, install_content_owner, refresh_identity, refresh_node_derivatives,
    structural_call_unit,
};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use omega_abstract_operations::AbstractOperation;
use psi_core::{ClaimId, StructuralDomainId};

#[test]
fn accepts_content_only_internal_claim_transfer_and_rejects_interface_corruption() {
    let mut baseline = structural_call_unit();
    install_content_owner(&mut baseline);
    let claim = id(1, ClaimId::new);
    for function in &mut baseline.functions {
        let root = function.structural_parameters[0].place;
        function
            .content_entry_claims
            .push(content_entry_claim(claim, root));
    }
    let AbstractOperation::CallUnit {
        claim_transfers, ..
    } = &mut baseline.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural Unit call")
    };
    claim_transfers.push(psi_terminal::ClaimTransfer {
        claim,
        argument_index: 0,
    });
    refresh_node_derivatives(&mut baseline, 0, 0, 0);
    validate_psi_optimization_unit(&baseline)
        .expect("content-only claims participate in the live transfer namespace");

    let mut missing_transfer = baseline.clone();
    let AbstractOperation::CallUnit {
        claim_transfers, ..
    } = &mut missing_transfer.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural Unit call")
    };
    claim_transfers.clear();
    refresh_node_derivatives(&mut missing_transfer, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&missing_transfer),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut substituted_projection = baseline.clone();
    substituted_projection.functions[0].content_entry_claims[0].projections[0]
        .algebra
        .parameter = "validation::substituted-content".into();
    refresh_identity(&mut substituted_projection);
    assert!(matches!(
        validate_psi_optimization_unit(&substituted_projection),
        Err(OptimizationUnitValidationError::ContentProjectionOwnerMismatch(_))
    ));

    let mutate_projection = [
        |projection: &mut psi_terminal::ClaimContentProjection| {
            projection.projection.domain = id(99, psi_core::ContentDomainId::new);
        },
        |projection: &mut psi_terminal::ClaimContentProjection| {
            projection.projection.projection_report_fingerprint ^= 1;
        },
        |projection: &mut psi_terminal::ClaimContentProjection| {
            projection.algebra.kind = psi_core::ContentAlgebraKind::IntervalSet;
        },
    ];
    for mutate in mutate_projection {
        let mut candidate = baseline.clone();
        mutate(&mut candidate.functions[0].content_entry_claims[0].projections[0]);
        refresh_identity(&mut candidate);
        assert!(matches!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::ContentProjectionOwnerMismatch(_))
        ));
    }

    let mut mismatched_interface = baseline.clone();
    let semantic_domain = id(2, psi_core::DomainSemanticId::new);
    let algebra = psi_core::ContentAlgebra {
        kind: psi_core::ContentAlgebraKind::CountedQuantity,
        parameter: "validation::alternate-content".into(),
    };
    let expression = psi_core::ContentProjectionExpression::CountedQuantity(
        psi_core::ContentProjectionScalar::Natural("2".into()),
    );
    let identity = psi_core::ContentProjectionIdentity {
        domain: id(semantic_domain.get(), psi_core::ContentDomainId::new),
        projection_report_fingerprint:
            psi_language_semantics::content::terminal_projection_report_fingerprint(
                &algebra,
                &expression,
            ),
    };
    let mut domains = mismatched_interface.structural_domains.to_vec();
    domains.push(psi_terminal::StructuralDomainDeclaration {
        id: id(2, StructuralDomainId::new),
        semantic_domain,
        identity: "validation::alternate-content-domain".into(),
        carrier: mismatched_interface.structural_types[0].id,
        content_projection: Some(psi_terminal::StructuralContentProjection {
            identity,
            algebra: algebra.clone(),
            expression,
        }),
    });
    mismatched_interface.structural_domains = domains.into();
    let callee_projection =
        &mut mismatched_interface.functions[1].content_entry_claims[0].projections[0];
    callee_projection.projection = identity;
    callee_projection.algebra = algebra;
    refresh_identity(&mut mismatched_interface);
    assert!(matches!(
        validate_psi_optimization_unit(&mismatched_interface),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));
}
