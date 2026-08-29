use super::shared::*;
use super::structural_types::*;

pub(super) fn encode_boundary_settlement(
    bytes: &mut Vec<u8>,
    settlement: &LegalizedBoundarySettlement,
) {
    bytes.extend_from_slice(&settlement.operation.get().to_le_bytes());
    bytes.extend_from_slice(&settlement.boundary.get().to_le_bytes());
    let execution = settlement.provider_execution;
    bytes.extend_from_slice(
        &execution
            .provider_plan_report_identity()
            .get()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&execution.provider_execution_report_identity().to_le_bytes());
    bytes.extend_from_slice(
        &execution
            .provider_execution_report_fingerprint()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&execution.normalized_root_report_identity().to_le_bytes());
    bytes.extend_from_slice(
        &execution
            .boundary_contract_report_fingerprint()
            .to_le_bytes(),
    );
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
        bytes.extend_from_slice(&receipt.claim.get().to_le_bytes());
        bytes.extend_from_slice(&receipt.argument_index.to_le_bytes());
    }
    encode_fuel(bytes, &settlement.fuel);
    encode_effect(bytes, settlement.effect);
    encode_ownership_roster(bytes, &settlement.ownership);
}

pub(super) fn encode_call_source(bytes: &mut Vec<u8>, source: &LegalizedCallUnitSource) {
    match source {
        LegalizedCallUnitSource::AuthoredCallUnit => bytes.push(1),
        LegalizedCallUnitSource::InstalledProvider {
            boundary,
            provider,
            completion_claim_sources,
            completion_receipts,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&boundary.get().to_le_bytes());
            encode_provider_candidate(bytes, provider);
            encode_len(bytes, completion_claim_sources.len());
            for source in completion_claim_sources {
                encode_completion_claim_source(bytes, source);
            }
            encode_len(bytes, completion_receipts.len());
            for receipt in completion_receipts {
                bytes.extend_from_slice(&receipt.claim.get().to_le_bytes());
                bytes.extend_from_slice(&receipt.argument_index.to_le_bytes());
            }
        }
    }
}

pub(super) fn encode_provider_candidate(
    bytes: &mut Vec<u8>,
    provider: &ProviderCandidateConformance,
) {
    bytes.extend_from_slice(&provider.boundary.get().to_le_bytes());
    encode_string(bytes, &provider.requirement_identity);
    encode_string(bytes, &provider.provider_identity);
    encode_string(bytes, &provider.candidate_identity);
    bytes.extend_from_slice(&provider.candidate.get().to_le_bytes());
    encode_len(bytes, provider.signature.parameters.len());
    for parameter in &provider.signature.parameters {
        bytes.extend_from_slice(&parameter.position.to_le_bytes());
        bytes.push(u8::from(parameter.is_self));
        bytes.extend_from_slice(&parameter.structural_type.get().to_le_bytes());
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
        bytes.extend_from_slice(&parameter.boundary_index.to_le_bytes());
        bytes.extend_from_slice(&parameter.candidate_index.to_le_bytes());
    }
    encode_len(bytes, provider.refinement.required_domains.len());
    for requirement in &provider.refinement.required_domains {
        bytes.extend_from_slice(&requirement.argument_index.to_le_bytes());
        bytes.extend_from_slice(&requirement.domain.get().to_le_bytes());
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
    bytes.extend_from_slice(&source.claim.get().to_le_bytes());
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
            bytes.extend_from_slice(&content.claim.get().to_le_bytes());
            bytes.push(match content.input.version {
                ContentPlaceVersion::Entry => 1,
                ContentPlaceVersion::Current => 2,
            });
            bytes.extend_from_slice(&content.input.root.get().to_le_bytes());
            encode_len(bytes, content.input.segments.len());
            for segment in &content.input.segments {
                match segment {
                    ContentPlaceSegment::Field(value) => {
                        bytes.push(1);
                        encode_string(bytes, value);
                    }
                    ContentPlaceSegment::FixedIndex(value) => {
                        bytes.push(2);
                        bytes.extend_from_slice(&value.to_le_bytes());
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
    bytes.extend_from_slice(&projection.projection.domain.get().to_le_bytes());
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
    bytes.extend_from_slice(&effect.input.to_le_bytes());
    bytes.extend_from_slice(&effect.output.to_le_bytes());
}

pub(super) fn encode_ownership_roster(bytes: &mut Vec<u8>, ownership: &[OwnershipEvent]) {
    encode_len(bytes, ownership.len());
    for event in ownership {
        match event {
            OwnershipEvent::ClaimTransfer(claims) => {
                bytes.push(1);
                encode_ids(bytes, claims.iter().map(|claim| claim.get()));
            }
            OwnershipEvent::ClaimCompletion(claims) => {
                bytes.push(2);
                encode_ids(bytes, claims.iter().map(|claim| claim.get()));
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
        }
    }
}

pub(super) fn encode_cleanup_action(
    bytes: &mut Vec<u8>,
    action: &psi_terminal::TerminalAffineCleanupAction,
) {
    match action {
        psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => {
            bytes.push(1);
            bytes.extend_from_slice(&place.get().to_le_bytes());
        }
        psi_terminal::TerminalAffineCleanupAction::DiscardResidual(discard) => {
            bytes.push(2);
            bytes.extend_from_slice(&discard.place.get().to_le_bytes());
            encode_structural_path(bytes, &discard.path);
            bytes.extend_from_slice(&discard.structural_type.get().to_le_bytes());
        }
        psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
            bytes.push(3);
            bytes.extend_from_slice(&cleanup.place.get().to_le_bytes());
            bytes.extend_from_slice(&cleanup.structural_type.get().to_le_bytes());
            bytes.extend_from_slice(&cleanup.cleanup_machine.get().to_le_bytes());
            encode_option_id(bytes, cleanup.cleanup_receiver.map(|place| place.get()));
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
