use super::encoding::{encode_fuel, encode_ids, encode_len};
use crate::legalized_operations::identity::structural_types::{
    encode_access, encode_entry_claim, encode_multiplicity, encode_string,
    encode_structural_argument, encode_structural_path,
};
use crate::legalized_operations::{LegalizedBoundarySettlement, NativeCallOrigin};
use abstract_operations::CompletionClaimSource;
use optimization_unit::{EffectLink, OwnershipEvent};
use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentPlaceSegment, ContentPlaceVersion,
};
use terminal_psi::{ClaimContentProjection, ProviderCandidateConformance};

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
    encode_len(bytes, settlement.completion_claim_sources.len());
    for source in &settlement.completion_claim_sources {
        encode_completion_claim_source(bytes, source);
    }
    encode_len(bytes, settlement.completion_receipts.len());
    for receipt in &settlement.completion_receipts {
        crate::codec_bytes::push_encoded(bytes, receipt.claim.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, receipt.argument_index.to_le_bytes());
        encode_structural_path(bytes, &receipt.path);
    }
    encode_fuel(bytes, &settlement.fuel);
    encode_effect(bytes, settlement.effect);
    encode_ownership_roster(bytes, &settlement.ownership);
}

/// The complete admitted provider-execution coordinate set, shared by
/// boundary settlements and evaluated normalized foreign calls.
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
            encode_len(bytes, completion_claim_sources.len());
            for source in completion_claim_sources {
                encode_completion_claim_source(bytes, source);
            }
            encode_len(bytes, completion_receipts.len());
            for receipt in completion_receipts {
                bytes.extend_from_slice(&receipt.claim.get().to_le_bytes());
                bytes.extend_from_slice(&receipt.argument_index.to_le_bytes());
                encode_structural_path(bytes, &receipt.path);
            }
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
            encode_len(bytes, completion_claim_sources.len());
            for source in completion_claim_sources {
                encode_completion_claim_source(bytes, source);
            }
            encode_len(bytes, completion_receipts.len());
            for receipt in completion_receipts {
                crate::codec_bytes::push_encoded(bytes, receipt.claim.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, receipt.argument_index.to_le_bytes());
                encode_structural_path(bytes, &receipt.path);
            }
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

pub(super) fn encode_completion_claim_source(bytes: &mut Vec<u8>, source: &CompletionClaimSource) {
    crate::codec_bytes::push_encoded(bytes, source.claim.get().to_le_bytes());
    match &source.entry {
        Some(entry) => {
            bytes.push(1);
            encode_entry_claim(bytes, entry);
        }
        None => bytes.push(0),
    }
    match &source.content {
        Some(content) => {
            bytes.push(1);
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
        }
        None => bytes.push(0),
    }
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
                encode_len(bytes, receipts.len());
                for receipt in receipts {
                    bytes.extend_from_slice(&receipt.claim.get().to_le_bytes());
                    bytes.extend_from_slice(&receipt.argument_index.to_le_bytes());
                    encode_structural_path(bytes, &receipt.path);
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{MachineId, PlaceId, StructuralTypeId};
    use terminal_psi::{NominalAffineCleanup, StructuralPathSegment, TerminalAffineCleanupAction};

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
}
