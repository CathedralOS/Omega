use omega_abstract_operations::CompletionClaimSource;
use omega_legalized_operations::{LegalizedBoundarySettlement, LegalizedCallUnitSource};
use omega_optimization_unit::{EffectLink, OwnershipEvent};
use omega_target_operations::{
    ClaimCompletionOnlyRealization, ProviderExecutionBinding, ProviderPlanReportIdentity,
};
use psi_core::{
    BoundaryMachineId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace,
    MachineId, ObligationId, PlaceId, StructuralTypeId,
};
use psi_terminal::{
    ClaimContentProjection, CompletionReceipt, ContentEntryClaim, TerminalAffineCleanupAction,
};

use crate::FixedViewCopyDecodeError;

use super::declarations::{
    decode_entry_claim, decode_path, decode_semantic_argument, decode_string, encode_entry_claim,
    encode_path, encode_semantic_argument, encode_string,
};
use super::provider;
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::{
    primitives::{Cursor, decode_id, decode_ids, encode_ids, length},
    selected::provenance::{decode_fuel, encode_fuel},
};

pub(super) fn encode_boundary_settlement(
    bytes: &mut Vec<u8>,
    settlement: &LegalizedBoundarySettlement,
) {
    bytes.extend_from_slice(&settlement.operation.get().to_le_bytes());
    bytes.extend_from_slice(&settlement.boundary.get().to_le_bytes());
    encode_provider_execution(bytes, settlement.provider_execution);
    let _ = settlement.realization;
    bytes.push(1);
    length(bytes, settlement.arguments.len());
    for argument in &settlement.arguments {
        encode_semantic_argument(bytes, argument);
    }
    length(bytes, settlement.completion_claim_sources.len());
    for source in &settlement.completion_claim_sources {
        encode_completion_claim_source(bytes, source);
    }
    length(bytes, settlement.completion_receipts.len());
    for receipt in &settlement.completion_receipts {
        encode_completion_receipt(bytes, *receipt);
    }
    encode_fuel(bytes, &settlement.fuel);
    encode_effect(bytes, settlement.effect);
    encode_ownership(bytes, &settlement.ownership);
}

pub(super) fn decode_boundary_settlement(
    cursor: &mut Cursor<'_>,
) -> Result<LegalizedBoundarySettlement, FixedViewCopyDecodeError> {
    let operation = decode_id(cursor, psi_core::OperationId::new)?;
    let boundary = decode_id(cursor, BoundaryMachineId::new)?;
    let provider_execution = decode_provider_execution(cursor)?;
    match cursor.byte()? {
        1 => {}
        tag => return Err(FixedViewCopyDecodeError::UnknownBoundaryRealization(tag)),
    }
    let argument_count = cursor.length()?;
    let mut arguments = Vec::with_capacity(argument_count.min(cursor.remaining()));
    for _ in 0..argument_count {
        arguments.push(decode_semantic_argument(cursor)?);
    }
    let source_count = cursor.length()?;
    let mut completion_claim_sources = Vec::with_capacity(source_count.min(cursor.remaining()));
    for _ in 0..source_count {
        completion_claim_sources.push(decode_completion_claim_source(cursor)?);
    }
    let receipt_count = cursor.length()?;
    let mut completion_receipts = Vec::with_capacity(receipt_count.min(cursor.remaining()));
    for _ in 0..receipt_count {
        completion_receipts.push(decode_completion_receipt(cursor)?);
    }
    Ok(LegalizedBoundarySettlement {
        operation,
        boundary,
        provider_execution,
        realization: ClaimCompletionOnlyRealization,
        arguments,
        completion_claim_sources,
        completion_receipts,
        fuel: decode_fuel(cursor)?,
        effect: decode_effect(cursor)?,
        ownership: decode_ownership(cursor)?,
    })
}

pub(super) fn encode_call_source(
    bytes: &mut Vec<u8>,
    source: &LegalizedCallUnitSource,
    retain_projected_qualifications: bool,
) {
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
            provider::encode(bytes, provider, retain_projected_qualifications);
            length(bytes, completion_claim_sources.len());
            for source in completion_claim_sources {
                encode_completion_claim_source(bytes, source);
            }
            length(bytes, completion_receipts.len());
            for receipt in completion_receipts {
                encode_completion_receipt(bytes, *receipt);
            }
        }
    }
}

pub(super) fn decode_call_source(
    cursor: &mut Cursor<'_>,
    retain_projected_qualifications: bool,
) -> Result<LegalizedCallUnitSource, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        1 => Ok(LegalizedCallUnitSource::AuthoredCallUnit),
        2 => {
            let boundary = decode_id(cursor, BoundaryMachineId::new)?;
            let provider = provider::decode(cursor, retain_projected_qualifications)?;
            let source_count = cursor.length()?;
            let mut completion_claim_sources =
                Vec::with_capacity(source_count.min(cursor.remaining()));
            for _ in 0..source_count {
                completion_claim_sources.push(decode_completion_claim_source(cursor)?);
            }
            let receipt_count = cursor.length()?;
            let mut completion_receipts = Vec::with_capacity(receipt_count.min(cursor.remaining()));
            for _ in 0..receipt_count {
                completion_receipts.push(decode_completion_receipt(cursor)?);
            }
            Ok(LegalizedCallUnitSource::InstalledProvider {
                boundary,
                provider,
                completion_claim_sources,
                completion_receipts,
            })
        }
        tag => Err(FixedViewCopyDecodeError::UnknownCallSource(tag)),
    }
}

fn encode_provider_execution(bytes: &mut Vec<u8>, execution: ProviderExecutionBinding) {
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
}

fn decode_provider_execution(
    cursor: &mut Cursor<'_>,
) -> Result<ProviderExecutionBinding, FixedViewCopyDecodeError> {
    let plan = ProviderPlanReportIdentity::new(cursor.u64()?)
        .ok_or(FixedViewCopyDecodeError::InvalidProviderExecution)?;
    ProviderExecutionBinding::from_execution_record(
        plan,
        cursor.u64()?,
        cursor.u64()?,
        cursor.u64()?,
        cursor.u64()?,
    )
    .ok_or(FixedViewCopyDecodeError::InvalidProviderExecution)
}

fn encode_completion_claim_source(bytes: &mut Vec<u8>, source: &CompletionClaimSource) {
    bytes.extend_from_slice(&source.claim.get().to_le_bytes());
    match &source.entry {
        None => bytes.push(0),
        Some(entry) => {
            bytes.push(1);
            encode_entry_claim(bytes, entry);
        }
    }
    match &source.content {
        None => bytes.push(0),
        Some(content) => {
            bytes.push(1);
            encode_content_entry_claim(bytes, content);
        }
    }
}

fn decode_completion_claim_source(
    cursor: &mut Cursor<'_>,
) -> Result<CompletionClaimSource, FixedViewCopyDecodeError> {
    let claim = decode_id(cursor, ClaimId::new)?;
    let entry = match cursor.byte()? {
        0 => None,
        1 => Some(decode_entry_claim(cursor)?),
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    let content = match cursor.byte()? {
        0 => None,
        1 => Some(decode_content_entry_claim(cursor)?),
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    Ok(CompletionClaimSource {
        claim,
        entry,
        content,
    })
}

fn encode_content_entry_claim(bytes: &mut Vec<u8>, content: &ContentEntryClaim) {
    bytes.extend_from_slice(&content.claim.get().to_le_bytes());
    encode_content_place(bytes, &content.input);
    length(bytes, content.projections.len());
    for projection in &content.projections {
        bytes.extend_from_slice(&projection.projection.domain.get().to_le_bytes());
        bytes.extend_from_slice(
            &projection
                .projection
                .projection_report_fingerprint
                .to_le_bytes(),
        );
        encode_content_algebra(bytes, &projection.algebra);
    }
}

fn decode_content_entry_claim(
    cursor: &mut Cursor<'_>,
) -> Result<ContentEntryClaim, FixedViewCopyDecodeError> {
    let claim = decode_id(cursor, ClaimId::new)?;
    let input = decode_content_place(cursor)?;
    let count = cursor.length()?;
    let mut projections = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        projections.push(ClaimContentProjection {
            projection: ContentProjectionIdentity {
                domain: decode_id(cursor, ContentDomainId::new)?,
                projection_report_fingerprint: cursor.u64()?,
            },
            algebra: decode_content_algebra(cursor)?,
        });
    }
    Ok(ContentEntryClaim {
        claim,
        input,
        projections,
    })
}

fn encode_content_place(bytes: &mut Vec<u8>, place: &ContentStructuralPlace) {
    bytes.push(match place.version {
        ContentPlaceVersion::Entry => 1,
        ContentPlaceVersion::Current => 2,
    });
    bytes.extend_from_slice(&place.root.get().to_le_bytes());
    length(bytes, place.segments.len());
    for segment in &place.segments {
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
}

fn decode_content_place(
    cursor: &mut Cursor<'_>,
) -> Result<ContentStructuralPlace, FixedViewCopyDecodeError> {
    let version = match cursor.byte()? {
        1 => ContentPlaceVersion::Entry,
        2 => ContentPlaceVersion::Current,
        tag => return Err(FixedViewCopyDecodeError::UnknownContentPlaceVersion(tag)),
    };
    let root = decode_id(cursor, PlaceId::new)?;
    let count = cursor.length()?;
    let mut segments = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        segments.push(match cursor.byte()? {
            1 => ContentPlaceSegment::Field(decode_string(cursor)?),
            2 => ContentPlaceSegment::FixedIndex(cursor.u64()?),
            3 => ContentPlaceSegment::Case(decode_string(cursor)?),
            tag => return Err(FixedViewCopyDecodeError::UnknownContentPlaceSegment(tag)),
        });
    }
    Ok(ContentStructuralPlace {
        version,
        root,
        segments,
    })
}

fn encode_content_algebra(bytes: &mut Vec<u8>, algebra: &ContentAlgebra) {
    bytes.push(match algebra.kind {
        ContentAlgebraKind::IntervalSet => 1,
        ContentAlgebraKind::CountedQuantity => 2,
    });
    encode_string(bytes, &algebra.parameter);
}

fn decode_content_algebra(
    cursor: &mut Cursor<'_>,
) -> Result<ContentAlgebra, FixedViewCopyDecodeError> {
    let kind = match cursor.byte()? {
        1 => ContentAlgebraKind::IntervalSet,
        2 => ContentAlgebraKind::CountedQuantity,
        tag => return Err(FixedViewCopyDecodeError::UnknownContentAlgebra(tag)),
    };
    Ok(ContentAlgebra {
        kind,
        parameter: decode_string(cursor)?,
    })
}

fn encode_completion_receipt(bytes: &mut Vec<u8>, receipt: CompletionReceipt) {
    bytes.extend_from_slice(&receipt.claim.get().to_le_bytes());
    bytes.extend_from_slice(&receipt.argument_index.to_le_bytes());
}

fn decode_completion_receipt(
    cursor: &mut Cursor<'_>,
) -> Result<CompletionReceipt, FixedViewCopyDecodeError> {
    Ok(CompletionReceipt {
        claim: decode_id(cursor, ClaimId::new)?,
        argument_index: cursor.u32()?,
    })
}

pub(super) fn encode_effect(bytes: &mut Vec<u8>, effect: EffectLink) {
    bytes.extend_from_slice(&effect.input.to_le_bytes());
    bytes.extend_from_slice(&effect.output.to_le_bytes());
}

pub(super) fn decode_effect(
    cursor: &mut Cursor<'_>,
) -> Result<EffectLink, FixedViewCopyDecodeError> {
    Ok(EffectLink {
        input: cursor.u64()?,
        output: cursor.u64()?,
    })
}

pub(super) fn encode_ownership(bytes: &mut Vec<u8>, ownership: &[OwnershipEvent]) {
    length(bytes, ownership.len());
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
                length(bytes, actions.len());
                for action in actions {
                    encode_cleanup(bytes, action);
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

pub(super) fn decode_ownership(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<OwnershipEvent>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut ownership = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        ownership.push(match cursor.byte()? {
            1 => OwnershipEvent::ClaimTransfer(decode_ids(cursor, ClaimId::new)?),
            2 => OwnershipEvent::ClaimCompletion(decode_ids(cursor, ClaimId::new)?),
            3 => {
                let action_count = cursor.length()?;
                let mut actions = Vec::with_capacity(action_count.min(cursor.remaining()));
                for _ in 0..action_count {
                    actions.push(decode_cleanup(cursor)?);
                }
                OwnershipEvent::Cleanup(actions)
            }
            4 => OwnershipEvent::StructuralReturn(decode_ids(cursor, ClaimId::new)?),
            5 => OwnershipEvent::CrashFrontier(decode_ids(cursor, ClaimId::new)?),
            tag => return Err(FixedViewCopyDecodeError::UnknownOwnershipEvent(tag)),
        });
    }
    Ok(ownership)
}

fn encode_cleanup(bytes: &mut Vec<u8>, action: &TerminalAffineCleanupAction) {
    match action {
        TerminalAffineCleanupAction::DiscardRoot(place) => {
            bytes.push(1);
            bytes.extend_from_slice(&place.get().to_le_bytes());
        }
        TerminalAffineCleanupAction::DiscardResidual(discard) => {
            bytes.push(2);
            bytes.extend_from_slice(&discard.place.get().to_le_bytes());
            encode_path(bytes, &discard.path);
            bytes.extend_from_slice(&discard.structural_type.get().to_le_bytes());
        }
        TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
            bytes.push(3);
            bytes.extend_from_slice(&cleanup.place.get().to_le_bytes());
            bytes.extend_from_slice(&cleanup.structural_type.get().to_le_bytes());
            bytes.extend_from_slice(&cleanup.cleanup_machine.get().to_le_bytes());
            match cleanup.cleanup_receiver {
                None => bytes.push(0),
                Some(place) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&place.get().to_le_bytes());
                }
            }
            encode_ids(
                bytes,
                cleanup
                    .requirement_obligations
                    .iter()
                    .map(|value| value.get()),
            );
        }
    }
}

fn decode_cleanup(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalAffineCleanupAction, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        1 => Ok(TerminalAffineCleanupAction::DiscardRoot(decode_id(
            cursor,
            PlaceId::new,
        )?)),
        2 => Ok(TerminalAffineCleanupAction::DiscardResidual(
            psi_terminal::StructuralAffineDiscard {
                place: decode_id(cursor, PlaceId::new)?,
                path: decode_path(cursor)?,
                structural_type: decode_id(cursor, StructuralTypeId::new)?,
            },
        )),
        3 => {
            let place = decode_id(cursor, PlaceId::new)?;
            let structural_type = decode_id(cursor, StructuralTypeId::new)?;
            let cleanup_machine = decode_id(cursor, MachineId::new)?;
            let cleanup_receiver = match cursor.byte()? {
                0 => None,
                1 => Some(decode_id(cursor, PlaceId::new)?),
                tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
            };
            Ok(TerminalAffineCleanupAction::InvokeNominal(
                psi_terminal::NominalAffineCleanup {
                    place,
                    structural_type,
                    cleanup_machine,
                    cleanup_receiver,
                    requirement_obligations: decode_ids(cursor, ObligationId::new)?,
                },
            ))
        }
        tag => Err(FixedViewCopyDecodeError::UnknownCleanupAction(tag)),
    }
}
