// Structural boundary settlements, call origins, completion custody, ownership rosters, and
// structural result declarations in canonical legalized-operation identities.
//
// `encode_boundary_settlement`, `encode_provider_execution`, and `encode_call_source` bind
// provider conformance, assumed-binding contracts, and completion claim/receipt rosters.
// `encode_ownership_roster` and `encode_cleanup_action` serialize ownership events and affine
// cleanup actions. `encode_operation_result` and `encode_result` bind operation-level and
// function-level structural result declarations, including reference result sources and leaf paths.

use super::structural_types::{
    encode_access, encode_entry_claim, encode_multiplicity, encode_projected_qualifications,
    encode_reference_origin, encode_string, encode_structural_argument, encode_structural_path,
};
use super::{encode_ids, encode_len, encode_option, encode_work};
use crate::{LegalizedBoundarySettlement, NativeCallOrigin};
use abstract_operations::CompletionClaimSource;
use optimization_unit::{EffectLink, OwnershipEvent};
use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentPlaceSegment, ContentPlaceVersion,
};
use terminal_psi::{
    ClaimContentProjection, ProviderCandidateConformance, StructuralOperationResult,
    StructuralResultDeclaration,
};

pub(super) fn encode_boundary_settlement(
    bytes: &mut Vec<u8>,
    settlement: &LegalizedBoundarySettlement,
) {
    crate::codec_bytes::push_encoded(bytes, settlement.operation.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, settlement.boundary.get().to_le_bytes());
    encode_provider_execution(bytes, settlement.provider_execution);
    bytes.push(1);
    encode_len(bytes, settlement.arguments.len());
    for argument in &settlement.arguments {
        encode_structural_argument(bytes, argument);
    }
    encode_completion_roster(
        bytes,
        &settlement.completion_claim_sources,
        &settlement.completion_receipts,
    );
    encode_work(bytes, &settlement.work);
    encode_effect(bytes, settlement.effect);
    encode_ownership_roster(bytes, &settlement.ownership);
}

pub fn encode_provider_execution(
    bytes: &mut Vec<u8>,
    execution: target_operations::ProviderExecutionBinding,
) {
    bytes.extend_from_slice(
        &execution
            .provider_plan_report_identity()
            .get()
            .to_le_bytes(),
    );
    crate::codec_bytes::push_encoded(
        bytes,
        execution.provider_execution_report_identity().to_le_bytes(),
    );
    bytes.extend_from_slice(
        &execution
            .provider_execution_report_fingerprint()
            .to_le_bytes(),
    );
    crate::codec_bytes::push_encoded(
        bytes,
        execution.normalized_root_report_identity().to_le_bytes(),
    );
    bytes.extend_from_slice(
        &execution
            .boundary_contract_report_fingerprint()
            .to_le_bytes(),
    );
}

pub(super) fn encode_call_source(bytes: &mut Vec<u8>, source: &NativeCallOrigin) {
    match source {
        NativeCallOrigin::Authored => bytes.push(1),
        NativeCallOrigin::ProgramEntry {
            boundary,
            entry,
            source_signature,
            completion_claim_sources,
            completion_receipts,
        } => {
            bytes.push(5);
            bytes.extend_from_slice(&boundary.get().to_le_bytes());
            bytes.extend_from_slice(&entry.get().to_le_bytes());
            bytes.extend_from_slice(source_signature);
            encode_completion_roster(bytes, completion_claim_sources, completion_receipts);
        }
        NativeCallOrigin::TargetFamily {
            boundary,
            member,
            target,
            completion_claim_sources,
            completion_receipts,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&boundary.get().to_le_bytes());
            bytes.extend_from_slice(&member.get().to_le_bytes());
            encode_string(bytes, target);
            encode_completion_roster(bytes, completion_claim_sources, completion_receipts);
        }
        NativeCallOrigin::InstalledProvider {
            boundary,
            provider,
            assumption,
            completion_claim_sources,
            completion_receipts,
        } => {
            bytes.push(if assumption.is_some() { 4 } else { 2 });
            crate::codec_bytes::push_encoded(bytes, boundary.get().to_le_bytes());
            encode_provider_candidate(bytes, provider);
            if let Some(row) = assumption {
                bytes.extend_from_slice(b"assumed-binding-contract\0");
                for id in [
                    row.required.get(),
                    row.supplied_requirement.get(),
                    row.selected_requirement.get(),
                    row.implementation.get(),
                ] {
                    bytes.extend_from_slice(&id.to_le_bytes());
                }
                encode_len(bytes, row.supply_path.len());
                for id in &row.supply_path {
                    bytes.extend_from_slice(&id.get().to_le_bytes());
                }
            }
            encode_completion_roster(bytes, completion_claim_sources, completion_receipts);
        }
    }
}

pub(super) fn encode_provider_candidate(
    bytes: &mut Vec<u8>,
    provider: &ProviderCandidateConformance,
) {
    crate::codec_bytes::push_encoded(bytes, provider.boundary.get().to_le_bytes());
    bytes.extend_from_slice(&provider.requirement_identity.0.to_le_bytes());
    bytes.extend_from_slice(&provider.source_requirement_identity.0.to_le_bytes());
    bytes.extend_from_slice(&provider.declaration_application.0.to_le_bytes());
    bytes.extend_from_slice(&provider.receiving_application.0.to_le_bytes());
    encode_len(bytes, provider.receiving_region_arguments.len());
    for region in &provider.receiving_region_arguments {
        bytes.extend_from_slice(&region.0.to_le_bytes());
    }
    bytes.extend_from_slice(&provider.provider_identity.0.to_le_bytes());
    bytes.extend_from_slice(&provider.candidate_identity.0.to_le_bytes());
    bytes.extend_from_slice(&provider.source_candidate_identity.0.to_le_bytes());
    bytes.push(u8::from(
        provider.source_candidate_package_identity.is_some(),
    ));
    if let Some(package) = provider.source_candidate_package_identity {
        bytes.extend_from_slice(&package.digest());
    }
    bytes.extend_from_slice(&provider.provider_declaration.identity().0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, provider.candidate.get().to_le_bytes());
    encode_len(bytes, provider.signature.parameters.len());
    for parameter in &provider.signature.parameters {
        crate::codec_bytes::push_encoded(bytes, parameter.position.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, parameter.structural_type.get().to_le_bytes());
        encode_multiplicity(bytes, parameter.multiplicity);
        encode_access(bytes, parameter.access);
        encode_ids(
            bytes,
            parameter
                .qualifications
                .iter()
                .map(|qualification| qualification.get()),
        );
    }
    encode_len(bytes, provider.refinement.positional_parameters.len());
    for parameter in &provider.refinement.positional_parameters {
        crate::codec_bytes::push_encoded(bytes, parameter.boundary_index.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, parameter.candidate_index.to_le_bytes());
    }
    encode_len(bytes, provider.refinement.required_domains.len());
    for requirement in &provider.refinement.required_domains {
        crate::codec_bytes::push_encoded(bytes, requirement.argument_index.to_le_bytes());
        encode_structural_path(bytes, &requirement.path);
        crate::codec_bytes::push_encoded(bytes, requirement.domain.get().to_le_bytes());
    }
    encode_ids(
        bytes,
        provider
            .refinement
            .realized_service_ceiling
            .iter()
            .map(|service| service.get()),
    );
}

fn encode_receipts(bytes: &mut Vec<u8>, receipts: &[terminal_psi::CompletionReceipt]) {
    encode_len(bytes, receipts.len());
    for receipt in receipts {
        crate::codec_bytes::push_encoded(bytes, receipt.claim.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, receipt.argument_index.to_le_bytes());
        encode_structural_path(bytes, &receipt.path);
    }
}

pub(super) fn encode_completion_roster(
    bytes: &mut Vec<u8>,
    claim_sources: &[CompletionClaimSource],
    receipts: &[terminal_psi::CompletionReceipt],
) {
    encode_len(bytes, claim_sources.len());
    for source in claim_sources {
        encode_completion_claim_source(bytes, source);
    }
    encode_receipts(bytes, receipts);
}

pub(super) fn encode_completion_claim_source(bytes: &mut Vec<u8>, source: &CompletionClaimSource) {
    crate::codec_bytes::push_encoded(bytes, source.claim.get().to_le_bytes());
    encode_option(bytes, source.entry.as_ref(), encode_entry_claim);
    encode_option(bytes, source.content.as_ref(), |bytes, content| {
        crate::codec_bytes::push_encoded(bytes, content.claim.get().to_le_bytes());
        bytes.push(match content.input.version {
            ContentPlaceVersion::Entry => 1,
            ContentPlaceVersion::Current => 2,
        });
        crate::codec_bytes::push_encoded(bytes, content.input.root.get().to_le_bytes());
        encode_len(bytes, content.input.segments.len());
        for segment in &content.input.segments {
            match segment {
                ContentPlaceSegment::Field(value) => {
                    bytes.push(1);
                    encode_string(bytes, value);
                }
                ContentPlaceSegment::FixedIndex(value) => {
                    bytes.push(2);
                    crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
                }
                ContentPlaceSegment::Case(value) => {
                    bytes.push(3);
                    encode_string(bytes, value);
                }
            }
        }
        encode_len(bytes, content.projections.len());
        for projection in &content.projections {
            encode_claim_projection(bytes, projection);
        }
    });
}

pub(super) fn encode_claim_projection(bytes: &mut Vec<u8>, projection: &ClaimContentProjection) {
    crate::codec_bytes::push_encoded(bytes, projection.projection.domain.get().to_le_bytes());
    bytes.extend_from_slice(
        &projection
            .projection
            .projection_report_fingerprint
            .to_le_bytes(),
    );
    encode_content_algebra(bytes, &projection.algebra);
}

pub(super) fn encode_content_algebra(bytes: &mut Vec<u8>, algebra: &ContentAlgebra) {
    bytes.push(match algebra.kind {
        ContentAlgebraKind::IntervalSet => 1,
        ContentAlgebraKind::CountedQuantity => 2,
    });
    encode_string(bytes, &algebra.parameter);
}

pub(super) fn encode_effect(bytes: &mut Vec<u8>, effect: EffectLink) {
    crate::codec_bytes::push_encoded(bytes, effect.input.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, effect.output.to_le_bytes());
}

pub(super) fn encode_ownership_roster(bytes: &mut Vec<u8>, ownership: &[OwnershipEvent]) {
    encode_len(bytes, ownership.len());
    for event in ownership {
        match event {
            OwnershipEvent::ClaimTransfer(claims) => {
                bytes.push(1);
                encode_ids(bytes, claims.iter().map(|claim| claim.get()));
            }
            OwnershipEvent::ClaimCompletion(receipts) => {
                bytes.push(2);
                encode_receipts(bytes, receipts);
            }
            OwnershipEvent::Cleanup(actions) => {
                bytes.push(3);
                encode_len(bytes, actions.len());
                for action in actions {
                    encode_cleanup_action(bytes, action);
                }
            }
            OwnershipEvent::StructuralReturn(claims) => {
                bytes.push(4);
                encode_ids(bytes, claims.iter().map(|claim| claim.get()));
            }
            OwnershipEvent::CrashFrontier(claims) => {
                bytes.push(5);
                encode_ids(bytes, claims.iter().map(|claim| claim.get()));
            }
            OwnershipEvent::ExternalCompletionFrontier(claims) => {
                bytes.push(6);
                encode_ids(bytes, claims.iter().map(|claim| claim.get()));
            }
        }
    }
}

pub(super) fn encode_cleanup_action(
    bytes: &mut Vec<u8>,
    action: &terminal_psi::TerminalAffineCleanupAction,
) {
    match action {
        terminal_psi::TerminalAffineCleanupAction::RetireHome(place) => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
        }
        terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
        }
        terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, discard.place.get().to_le_bytes());
            encode_structural_path(bytes, &discard.path);
            crate::codec_bytes::push_encoded(bytes, discard.structural_type.get().to_le_bytes());
        }
        terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
            bytes.push(if cleanup.retains_descendants { 5 } else { 3 });
            crate::codec_bytes::push_encoded(bytes, cleanup.place.get().to_le_bytes());
            encode_structural_path(bytes, &cleanup.path);
            crate::codec_bytes::push_encoded(bytes, cleanup.structural_type.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, cleanup.cleanup_machine.get().to_le_bytes());
            encode_ids(
                bytes,
                cleanup.consumed_claims.iter().map(|claim| claim.get()),
            );
            optimization_core::encode_option_u64(
                bytes,
                cleanup.cleanup_receiver.map(|place| place.get()),
            );
            encode_ids(
                bytes,
                cleanup
                    .requirement_obligations
                    .iter()
                    .map(|obligation| obligation.get()),
            );
        }
    }
}

pub(super) fn encode_operation_result(bytes: &mut Vec<u8>, result: &StructuralOperationResult) {
    crate::codec_bytes::push_encoded(bytes, result.place.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, result.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, result.multiplicity);
    encode_ids(
        bytes,
        result.qualifications.iter().map(|domain| domain.get()),
    );
    encode_projected_qualifications(bytes, &result.projected_qualifications);
    encode_projected_qualifications(bytes, &result.additional_input_qualifications);
    encode_len(bytes, result.claims.len());
    for claim in &result.claims {
        crate::codec_bytes::push_encoded(bytes, claim.claim.get().to_le_bytes());
        encode_structural_path(bytes, &claim.path);
    }
}

pub(super) fn encode_result(bytes: &mut Vec<u8>, result: &StructuralResultDeclaration) {
    crate::codec_bytes::push_encoded(bytes, result.place.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, result.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, result.multiplicity);
    encode_ids(
        bytes,
        result.qualifications.iter().map(|domain| domain.get()),
    );
    encode_projected_qualifications(bytes, &result.projected_qualifications);
    encode_len(bytes, result.reference_sources.len());
    for reference in &result.reference_sources {
        encode_structural_path(bytes, &reference.path);
        encode_len(bytes, reference.sources.len());
        for source in &reference.sources {
            encode_reference_origin(bytes, source);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
    use terminal_psi::{
        NominalAffineCleanup, StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralPathSegment, StructuralReferenceResultSource, TerminalAffineCleanupAction,
    };

    #[test]
    fn nominal_cleanup_identity_distinguishes_owned_subtrees() {
        let mut previous_encoding = None;
        for path in [
            Vec::new(),
            vec![StructuralPathSegment::Field(
                terminal_psi::TerminalIdentityIndex(1),
            )],
            vec![
                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(1)),
                StructuralPathSegment::FixedIndex(1),
            ],
        ] {
            let action = TerminalAffineCleanupAction::InvokeNominal(NominalAffineCleanup {
                retains_descendants: false,
                place: PlaceId::new(1).unwrap(),
                path,
                structural_type: StructuralTypeId::new(2).unwrap(),
                cleanup_machine: MachineId::new(3).unwrap(),
                consumed_claims: Vec::new(),
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            });
            let mut encoded = Vec::new();
            encode_cleanup_action(&mut encoded, &action);
            if let Some(previous) = previous_encoding {
                assert_ne!(encoded, previous);
            }
            previous_encoding = Some(encoded);
        }
    }

    fn parameter(
        source: &mut terminal_psi::StructuralReferenceOrigin,
    ) -> &mut terminal_psi::ReferenceOriginSource {
        match source {
            terminal_psi::StructuralReferenceOrigin::Parameter(source) => source,
            terminal_psi::StructuralReferenceOrigin::Invocation { .. } => {
                panic!("expected parameter origin")
            }
        }
    }

    #[test]
    fn reference_result_identity_retains_exact_origin_and_leaf_path() {
        let result = StructuralResultDeclaration {
            place: PlaceId::new(1).unwrap(),
            structural_type: StructuralTypeId::new(1).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: vec![StructuralReferenceResultSource {
                path: Vec::new(),
                sources: vec![
                    StructuralArgument {
                        place: PlaceId::new(2).unwrap(),
                        path: Vec::new(),
                        access: StructuralAccess::MutableBorrow,
                    }
                    .into(),
                    StructuralArgument {
                        place: PlaceId::new(4).unwrap(),
                        path: Vec::new(),
                        access: StructuralAccess::MutableBorrow,
                    }
                    .into(),
                ],
            }],
        };
        let identity = |result: &StructuralResultDeclaration| {
            let mut bytes = Vec::new();
            encode_result(&mut bytes, result);
            bytes
        };
        let original = identity(&result);
        for mutation in 0..13 {
            let mut changed = result.clone();
            match mutation {
                0 => changed.reference_sources.clear(),
                1 => {
                    parameter(&mut changed.reference_sources[0].sources[0]).place =
                        PlaceId::new(3).unwrap()
                }
                2 => parameter(&mut changed.reference_sources[0].sources[0])
                    .path
                    .push(terminal_psi::ReferenceOriginProjection::Exact(
                        StructuralPathSegment::Referent,
                    )),
                3 => changed.reference_sources[0]
                    .path
                    .push(StructuralPathSegment::Field(
                        terminal_psi::TerminalIdentityIndex(2),
                    )),
                4 => {
                    parameter(&mut changed.reference_sources[0].sources[0]).access =
                        StructuralAccess::SharedBorrow
                }
                5 => {
                    parameter(&mut changed.reference_sources[0].sources[1]).place =
                        PlaceId::new(5).unwrap()
                }
                6 => changed.reference_sources[0].sources.swap(0, 1),
                7 => {
                    changed.reference_sources[0].sources.pop();
                }
                8 => parameter(&mut changed.reference_sources[0].sources[0])
                    .path
                    .push(terminal_psi::ReferenceOriginProjection::Element),
                9 => {
                    parameter(&mut changed.reference_sources[0].sources[0]).extent =
                        semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                }
                10..13 => {
                    changed.reference_sources[0].sources[0] =
                        terminal_psi::StructuralReferenceOrigin::Invocation {
                            carrier_type: StructuralTypeId::new(if mutation == 11 { 3 } else { 2 })
                                .unwrap(),
                            path: if mutation == 12 {
                                vec![terminal_psi::ReferenceOriginProjection::Element]
                            } else {
                                Vec::new()
                            },
                            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                            access: StructuralAccess::MutableBorrow,
                        };
                }
                _ => unreachable!(),
            }
            assert_ne!(identity(&changed), original, "custody mutation {mutation}");
        }
    }
}
