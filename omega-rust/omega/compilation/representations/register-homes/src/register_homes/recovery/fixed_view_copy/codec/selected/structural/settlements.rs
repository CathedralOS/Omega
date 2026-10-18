use abstract_operations::CompletionClaimSource;
use legalized_operations::{
    LegalizedBoundarySettlement, NativeCallOrigin, encode_provider_execution,
};
use optimization_unit::{EffectLink, OwnershipEvent};
use selected_instructions::effects::program::identity::{encode_ownership, encode_path};
use semantic_vocabulary::{
    BoundaryMachineId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace,
    MachineId, ObligationId, PlaceId, StructuralTypeId,
};
use target_operations::{
    ClaimCompletionOnlyRealization, ProviderExecutionBinding, ProviderPlanReportIdentity,
};
use terminal_psi::{
    ClaimContentProjection, CompletionReceipt, ContentEntryClaim, TerminalAffineCleanupAction,
};

use crate::FixedViewCopyDecodeError;

use super::declarations::{
    decode_entry_claim, decode_path, decode_semantic_argument, decode_string, encode_entry_claim,
    encode_semantic_argument, encode_string,
};
use super::provider;
use crate::register_homes::recovery::fixed_view_copy::codec::{
    primitives::{Cursor, decode_id, decode_ids, decode_list, length},
    selected::provenance::{decode_fuel, encode_fuel},
};

pub(super) fn encode_boundary_settlement(
    bytes: &mut Vec<u8>,
    payload: &selected_instructions::SelectedBoundarySettlementPayload,
) -> Result<(), terminal_codec::CodecError> {
    let settlement = match payload {
        selected_instructions::SelectedBoundarySettlementPayload::HostedReadByte {
            operation,
            boundary,
            result,
            layout,
        } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, boundary.get().to_le_bytes());
            super::read_result::encode(bytes, result, layout);
            return Ok(());
        }
        selected_instructions::SelectedBoundarySettlementPayload::HostedExitProcessI32 {
            edge,
            execution,
            boundary,
            source,
        } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
            match execution {
                target_operations::BoundaryExecutionBinding::AdmittedProvider(provider) => {
                    bytes.push(0);
                    encode_provider_execution(bytes, *provider);
                }
                target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                    target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
                ) => bytes.push(1),
                _ => {
                    return Err(terminal_codec::CodecError::MalformedStructuralFoundation(
                        "process exit settlement has an incompatible execution binding",
                    ));
                }
            }
            crate::codec_bytes::push_encoded(bytes, boundary.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
            return Ok(());
        }
        selected_instructions::SelectedBoundarySettlementPayload::ClaimCompletion(settlement) => {
            bytes.push(0);
            settlement
        }
        selected_instructions::SelectedBoundarySettlementPayload::HostedWriteByteI32 {
            operation,
            boundary,
            source,
        } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, boundary.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
            return Ok(());
        }
    };
    crate::codec_bytes::push_encoded(bytes, settlement.operation.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, settlement.boundary.get().to_le_bytes());
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
        encode_completion_receipt(bytes, receipt);
    }
    encode_fuel(bytes, &settlement.fuel);
    encode_effect(bytes, settlement.effect);
    encode_ownership(bytes, &settlement.ownership);
    Ok(())
}

pub(super) fn decode_boundary_settlement(
    cursor: &mut Cursor<'_>,
) -> Result<selected_instructions::SelectedBoundarySettlementPayload, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        3 => {
            let operation = decode_id(cursor, semantic_vocabulary::OperationId::new)?;
            let boundary = decode_id(cursor, BoundaryMachineId::new)?;
            let (result, layout) = super::read_result::decode(cursor)?;
            return Ok(
                selected_instructions::SelectedBoundarySettlementPayload::HostedReadByte {
                    operation,
                    boundary,
                    result,
                    layout,
                },
            );
        }
        2 => {
            return Ok(
                selected_instructions::SelectedBoundarySettlementPayload::HostedExitProcessI32 {
                    edge: decode_id(cursor, semantic_vocabulary::EdgeId::new)?,
                    execution: match cursor.byte()? {
                        0 => target_operations::BoundaryExecutionBinding::AdmittedProvider(
                            decode_provider_execution(cursor)?,
                        ),
                        1 => target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                            target_operations::CompilerBuiltinExecution::HostedExitProcessI32,
                        ),
                        tag => {
                            return Err(FixedViewCopyDecodeError::UnknownBoundaryRealization(tag));
                        }
                    },
                    boundary: decode_id(cursor, BoundaryMachineId::new)?,
                    source: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                },
            );
        }
        0 => {}
        1 => {
            return Ok(
                selected_instructions::SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                    operation: decode_id(cursor, semantic_vocabulary::OperationId::new)?,
                    boundary: decode_id(cursor, BoundaryMachineId::new)?,
                    source: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                },
            );
        }
        tag => return Err(FixedViewCopyDecodeError::UnknownBoundaryRealization(tag)),
    }
    let operation = decode_id(cursor, semantic_vocabulary::OperationId::new)?;
    let boundary = decode_id(cursor, BoundaryMachineId::new)?;
    let provider_execution = decode_provider_execution(cursor)?;
    match cursor.byte()? {
        1 => {}
        tag => return Err(FixedViewCopyDecodeError::UnknownBoundaryRealization(tag)),
    }
    let arguments = decode_list(cursor, decode_semantic_argument)?;
    let completion_claim_sources = decode_list(cursor, decode_completion_claim_source)?;
    let completion_receipts = decode_list(cursor, decode_completion_receipt)?;
    Ok(
        selected_instructions::SelectedBoundarySettlementPayload::ClaimCompletion(
            LegalizedBoundarySettlement {
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
            },
        ),
    )
}

pub(super) fn encode_call_source(
    bytes: &mut Vec<u8>,
    source: &NativeCallOrigin,
    retain_projected_qualifications: bool,
) {
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
            length(bytes, completion_claim_sources.len());
            for source in completion_claim_sources {
                encode_completion_claim_source(bytes, source);
            }
            length(bytes, completion_receipts.len());
            for receipt in completion_receipts {
                encode_completion_receipt(bytes, receipt);
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
            if let Some(row) = assumption {
                for id in [
                    row.required.get(),
                    row.supplied_requirement.get(),
                    row.selected_requirement.get(),
                    row.implementation.get(),
                ] {
                    bytes.extend_from_slice(&id.to_le_bytes());
                }
                length(bytes, row.supply_path.len());
                for id in &row.supply_path {
                    bytes.extend_from_slice(&id.get().to_le_bytes());
                }
            }
            crate::codec_bytes::push_encoded(bytes, boundary.get().to_le_bytes());
            provider::encode(bytes, provider, retain_projected_qualifications);
            length(bytes, completion_claim_sources.len());
            for source in completion_claim_sources {
                encode_completion_claim_source(bytes, source);
            }
            length(bytes, completion_receipts.len());
            for receipt in completion_receipts {
                encode_completion_receipt(bytes, receipt);
            }
        }
    }
}

pub(super) fn decode_call_source(
    cursor: &mut Cursor<'_>,
    retain_projected_qualifications: bool,
) -> Result<NativeCallOrigin, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        1 => Ok(NativeCallOrigin::Authored),
        3 => {
            let boundary = decode_id(cursor, BoundaryMachineId::new)?;
            let member = decode_id(cursor, MachineId::new)?;
            let target = decode_string(cursor)?;
            let completion_claim_sources = decode_list(cursor, decode_completion_claim_source)?;
            let completion_receipts = decode_list(cursor, decode_completion_receipt)?;
            Ok(NativeCallOrigin::TargetFamily {
                boundary,
                member,
                target,
                completion_claim_sources,
                completion_receipts,
            })
        }
        tag @ (2 | 4) => {
            let assumption = if tag == 4 {
                Some(target_operations::InstalledAssumedBindingEvidence {
                    required: decode_id(cursor, BoundaryMachineId::new)?,
                    supplied_requirement: decode_id(cursor, BoundaryMachineId::new)?,
                    selected_requirement: decode_id(cursor, BoundaryMachineId::new)?,
                    implementation: decode_id(cursor, MachineId::new)?,
                    supply_path: decode_list(cursor, |cursor| {
                        decode_id(cursor, BoundaryMachineId::new)
                    })?,
                })
            } else {
                None
            };
            let boundary = decode_id(cursor, BoundaryMachineId::new)?;
            let provider = provider::decode(cursor, retain_projected_qualifications)?;
            let source_count = cursor.length()?;
            let mut completion_claim_sources =
                Vec::with_capacity(source_count.min(cursor.remaining()));
            for _ in 0..source_count {
                completion_claim_sources.push(decode_completion_claim_source(cursor)?);
            }
            let completion_receipts = decode_list(cursor, decode_completion_receipt)?;
            Ok(NativeCallOrigin::InstalledProvider {
                boundary,
                provider,
                assumption,
                completion_claim_sources,
                completion_receipts,
            })
        }
        tag => Err(FixedViewCopyDecodeError::UnknownCallSource(tag)),
    }
}

pub(super) fn decode_provider_execution(
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
    crate::codec_bytes::push_encoded(bytes, source.claim.get().to_le_bytes());
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
    let entry = crate::cursor::optional(
        cursor,
        |c| Ok(decode_entry_claim(c)?),
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    let content = crate::cursor::optional(
        cursor,
        |c| Ok(decode_content_entry_claim(c)?),
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    Ok(CompletionClaimSource {
        claim,
        entry,
        content,
    })
}

fn encode_content_entry_claim(bytes: &mut Vec<u8>, content: &ContentEntryClaim) {
    crate::codec_bytes::push_encoded(bytes, content.claim.get().to_le_bytes());
    encode_content_place(bytes, &content.input);
    length(bytes, content.projections.len());
    for projection in &content.projections {
        crate::codec_bytes::push_encoded(bytes, projection.projection.domain.get().to_le_bytes());
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
    crate::codec_bytes::push_encoded(bytes, place.root.get().to_le_bytes());
    length(bytes, place.segments.len());
    for segment in &place.segments {
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

fn encode_completion_receipt(bytes: &mut Vec<u8>, receipt: &CompletionReceipt) {
    crate::codec_bytes::push_encoded(bytes, receipt.claim.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, receipt.argument_index.to_le_bytes());
    encode_path(bytes, &receipt.path);
}

fn decode_completion_receipt(
    cursor: &mut Cursor<'_>,
) -> Result<CompletionReceipt, FixedViewCopyDecodeError> {
    Ok(CompletionReceipt {
        claim: decode_id(cursor, ClaimId::new)?,
        argument_index: cursor.u32()?,
        path: decode_path(cursor)?,
    })
}

pub(super) fn encode_effect(bytes: &mut Vec<u8>, effect: EffectLink) {
    crate::codec_bytes::push_encoded(bytes, effect.input.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, effect.output.to_le_bytes());
}

pub(super) fn decode_effect(
    cursor: &mut Cursor<'_>,
) -> Result<EffectLink, FixedViewCopyDecodeError> {
    Ok(EffectLink {
        input: cursor.u64()?,
        output: cursor.u64()?,
    })
}

pub(super) fn decode_ownership(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<OwnershipEvent>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut ownership = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        ownership.push(match cursor.byte()? {
            1 => OwnershipEvent::ClaimTransfer(decode_ids(cursor, ClaimId::new)?),
            2 => OwnershipEvent::ClaimCompletion(decode_list(cursor, decode_completion_receipt)?),
            3 => {
                let actions = decode_list(cursor, decode_cleanup)?;
                OwnershipEvent::Cleanup(actions)
            }
            4 => OwnershipEvent::StructuralReturn(decode_ids(cursor, ClaimId::new)?),
            5 => OwnershipEvent::CrashFrontier(decode_ids(cursor, ClaimId::new)?),
            6 => OwnershipEvent::ExternalCompletionFrontier(decode_ids(cursor, ClaimId::new)?),
            tag => return Err(FixedViewCopyDecodeError::UnknownOwnershipEvent(tag)),
        });
    }
    Ok(ownership)
}

pub(in super::super) fn decode_cleanup(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalAffineCleanupAction, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        4 => Ok(TerminalAffineCleanupAction::RetireHome(decode_id(
            cursor,
            PlaceId::new,
        )?)),
        1 => Ok(TerminalAffineCleanupAction::DiscardRoot(decode_id(
            cursor,
            PlaceId::new,
        )?)),
        2 => Ok(TerminalAffineCleanupAction::DiscardResidual(
            terminal_psi::StructuralAffineDiscard {
                place: decode_id(cursor, PlaceId::new)?,
                path: decode_path(cursor)?,
                structural_type: decode_id(cursor, StructuralTypeId::new)?,
            },
        )),
        tag @ (3 | 5) => {
            let place = decode_id(cursor, PlaceId::new)?;
            let path = decode_path(cursor)?;
            let structural_type = decode_id(cursor, StructuralTypeId::new)?;
            let cleanup_machine = decode_id(cursor, MachineId::new)?;
            let consumed_claims = decode_ids(cursor, ClaimId::new)?;
            let cleanup_receiver = match cursor.byte()? {
                0 => None,
                1 => Some(decode_id(cursor, PlaceId::new)?),
                tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
            };
            Ok(TerminalAffineCleanupAction::InvokeNominal(
                terminal_psi::NominalAffineCleanup {
                    retains_descendants: tag == 5,
                    place,
                    path,
                    structural_type,
                    cleanup_machine,
                    consumed_claims,
                    cleanup_receiver,
                    requirement_obligations: decode_ids(cursor, ObligationId::new)?,
                },
            ))
        }
        tag => Err(FixedViewCopyDecodeError::UnknownCleanupAction(tag)),
    }
}
