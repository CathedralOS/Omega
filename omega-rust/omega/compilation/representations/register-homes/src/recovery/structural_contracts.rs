/*
Structural contract, function metadata, and boundary settlement codecs for register-home recovery.

Once the recovery codec has decoded structural type declarations and call descriptors, it must assemble the complete function contract (`encode_contracts` / `decode_contracts`), structural signature (`encode_signature` / `decode_signature`), local storage and outgoing argument slots, memory access descriptors, and boundary settlements (`encode_boundary_settlement` / `decode_boundary_settlement`). Recovery preserves the full structural type catalog—including quotient representations, access views, placed input demands, and access lifecycles—because storage declarations alone cannot reconstruct view demands or lifecycle plans after register allocation.
*/

use abstract_operations::{CompletionClaimSource, StructuralTypeCatalog};
use legalized_operations::{
    LegalizedBoundarySettlement, LegalizedStructuralContract, NativeCallOrigin,
    encode_provider_execution,
};
use optimization_unit::{EffectLink, OwnershipEvent};
use selected_instructions::effects::program::identity::{
    encode_ownership, encode_path, encode_reference_origin_path,
};
use selected_instructions::{
    LocalStorageSlotId, OutgoingArgumentSlotId, SelectedBlockId, SelectedBoundarySettlement,
    SelectedBoundarySettlementPayload, SelectedFunction, SelectedInstructionId,
    SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    SelectedOutgoingArgumentSlot,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentDomainId,
    ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace,
    EdgeId, InvocationSite, MachineId, ObligationId, OperationId, PlaceId, ReferenceOriginExtent,
    ServiceId, StructuralDomainId, StructuralTypeId,
};
use target_operations::{
    ClaimCompletionOnlyRealization, ProviderExecutionBinding, ProviderPlanReportIdentity,
};
use terminal_codec::{encode_access_lifecycle_catalog, encode_access_view_input_catalog};
use terminal_psi::{
    ClaimContentProjection, CompletionReceipt, ContentEntryClaim, StructuralReferenceOrigin,
    TerminalAffineCleanupAction,
};

use super::fixed_view_copy_codec::{
    Cursor, decode_accepted_fact, decode_bool, decode_id, decode_ids, decode_list,
    decode_option_u64, decode_work, encode_ids, encode_work, length,
};
pub(super) use super::structural_calls::{
    decode_call, decode_normalized_foreign_call, decode_provider, decode_read_result_and_layout,
    decode_runtime_indices, encode_call, encode_normalized_foreign_call, encode_provider,
    encode_read_result_and_layout, encode_runtime_indices,
};
pub(super) use super::structural_declarations::{
    decode_access, decode_entry_claim, decode_multiplicity, decode_parameter, decode_path,
    decode_place, decode_projected, decode_reference_origin_path, decode_semantic_argument,
    decode_string, decode_type, encode_access, encode_entry_claim, encode_multiplicity,
    encode_parameter, encode_place, encode_projected, encode_semantic_argument, encode_string,
    encode_type,
};
use crate::FixedViewCopyDecodeError;
use crate::codec_bytes::push_encoded;
use crate::cursor::optional;

pub(super) fn decode_local_slot(
    cursor: &mut Cursor<'_>,
) -> Result<LocalStorageSlotId, FixedViewCopyDecodeError> {
    let tag = cursor.byte()?;
    if tag == 8 || tag == 9 {
        let edge = decode_id(cursor, EdgeId::new)?;
        return Ok(if tag == 8 {
            LocalStorageSlotId::InvocationResult { edge }
        } else {
            LocalStorageSlotId::InvocationArgument {
                edge,
                argument_index: cursor.u32()?,
            }
        });
    }
    if tag == 6 {
        return Ok(LocalStorageSlotId::EdgeSnapshot {
            edge: decode_id(cursor, EdgeId::new)?,
            binding_index: cursor.u32()?,
        });
    }
    if tag == 4 {
        return Ok(LocalStorageSlotId::StructuralParameter {
            place: decode_id(cursor, PlaceId::new)?,
        });
    }
    if tag == 3 {
        return Ok(LocalStorageSlotId::StructuralBlockParameter {
            block: decode_id(cursor, BlockId::new)?,
            place: decode_id(cursor, PlaceId::new)?,
        });
    }
    if tag == 2 {
        return Ok(LocalStorageSlotId::Spill {
            register: selected_instructions::VirtualRegisterId(cursor.u32()?),
        });
    }
    let operation = decode_id(cursor, OperationId::new)?;
    match tag {
        0 => Ok(LocalStorageSlotId::Structural {
            operation,
            place: decode_id(cursor, PlaceId::new)?,
        }),
        1 => Ok(LocalStorageSlotId::Boundary { operation }),
        7 => Ok(LocalStorageSlotId::BoundaryResult { operation }),
        5 => Ok(LocalStorageSlotId::StructuralCallArgument {
            operation,
            argument_index: cursor.u32()?,
        }),
        tag => Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    }
}

pub(super) fn decode_invocation_site(
    cursor: &mut Cursor<'_>,
) -> Result<InvocationSite, FixedViewCopyDecodeError> {
    let tag = cursor.byte()?;
    let identity = cursor.u64()?;
    InvocationSite::from_tagged_identity(tag, identity).ok_or(match tag {
        0 | 1 => FixedViewCopyDecodeError::InvalidSemanticId(identity),
        _ => FixedViewCopyDecodeError::UnknownOption(tag),
    })
}

fn encode_slot(bytes: &mut Vec<u8>, slot: OutgoingArgumentSlotId) {
    slot.encode_identity(bytes);
}

fn decode_slot(
    cursor: &mut Cursor<'_>,
) -> Result<OutgoingArgumentSlotId, FixedViewCopyDecodeError> {
    Ok(OutgoingArgumentSlotId {
        invocation_site: decode_invocation_site(cursor)?,
        argument_index: cursor.u32()?,
        role: match cursor.byte()? {
            0 => selected_instructions::OutgoingArgumentSlotRole::Argument,
            1 => selected_instructions::OutgoingArgumentSlotRole::ValueCopy,
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        },
    })
}

pub(super) fn encode_contracts(
    bytes: &mut Vec<u8>,
    function: &SelectedFunction,
) -> Result<(), terminal_codec::CodecError> {
    match &function.structural {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            encode_signature(bytes, value)?;
        }
    }
    length(bytes, function.local_storage_slots.len());
    for slot in &function.local_storage_slots {
        slot.id.encode_identity(bytes);
        push_encoded(bytes, slot.byte_size.to_le_bytes());
        push_encoded(bytes, slot.alignment.to_le_bytes());
    }
    length(bytes, function.outgoing_arguments.len());
    for row in &function.outgoing_arguments {
        encode_slot(bytes, row.id);
        push_encoded(bytes, row.byte_size.to_le_bytes());
        push_encoded(bytes, row.alignment.to_le_bytes());
        push_encoded(bytes, row.abi_stack_byte_offset.to_le_bytes());
    }
    length(bytes, function.calls.len());
    for row in &function.calls {
        encode_call(bytes, row);
    }
    length(bytes, function.normalized_foreign_calls.len());
    for row in &function.normalized_foreign_calls {
        encode_normalized_foreign_call(bytes, row);
    }
    length(bytes, function.memory_accesses.len());
    for row in &function.memory_accesses {
        selected_instructions::plan_identity::encode_memory_access(bytes, row);
    }
    length(bytes, function.boundary_settlements.len());
    for row in &function.boundary_settlements {
        push_encoded(bytes, row.block.0.to_le_bytes());
        push_encoded(bytes, row.instruction_index.to_le_bytes());
        encode_boundary_settlement(bytes, &row.settlement)?;
    }
    Ok(())
}

pub(super) fn decode_contracts(
    cursor: &mut Cursor<'_>,
    function: &mut SelectedFunction,
) -> Result<(), FixedViewCopyDecodeError> {
    function.structural = optional(
        cursor,
        decode_signature,
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    function.local_storage_slots = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(selected_instructions::SelectedLocalStorageSlot {
            id: decode_local_slot(cursor)?,
            byte_size: cursor.u32()?,
            alignment: cursor.u16()?,
        })
    })?;
    function.outgoing_arguments = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(SelectedOutgoingArgumentSlot {
            id: decode_slot(cursor)?,
            byte_size: cursor.u32()?,
            alignment: cursor.u16()?,
            abi_stack_byte_offset: cursor.u32()?,
        })
    })?;
    function.calls = decode_list(cursor, decode_call)?;
    function.normalized_foreign_calls = decode_list(cursor, decode_normalized_foreign_call)?;
    let count = cursor.length()?;
    for _ in 0..count {
        let instruction = SelectedInstructionId(cursor.u32()?);
        let origin = match cursor.byte()? {
            0 => SelectedMemoryAccessOrigin::Operation(decode_id(cursor, OperationId::new)?),
            1 => SelectedMemoryAccessOrigin::Edge(decode_id(cursor, EdgeId::new)?),
            2 => SelectedMemoryAccessOrigin::Block(decode_id(cursor, BlockId::new)?),
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        };
        let place = decode_id(cursor, PlaceId::new)?;
        let byte_offset = cursor.u32()?;
        let byte_count = cursor.u32()?;
        let role = match cursor.byte()? {
            tag @ (8 | 9) => {
                let length = decode_id(cursor, semantic_vocabulary::ValueId::new)?;
                let obligation = decode_id(cursor, semantic_vocabulary::ObligationId::new)?;
                let accepted_fact =
                    optimization_core::AcceptedObligationFactIdentity::from_bytes(cursor.array()?);
                if tag == 8 {
                    SelectedMemoryAccessRole::ReadByteSpan {
                        length,
                        obligation,
                        accepted_fact,
                    }
                } else {
                    SelectedMemoryAccessRole::WriteByteSpan {
                        length,
                        obligation,
                        accepted_fact,
                    }
                }
            }
            7 => SelectedMemoryAccessRole::WriteByteSequence {
                index: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                value: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                length: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                obligation: decode_id(cursor, semantic_vocabulary::ObligationId::new)?,
                accepted_fact: decode_accepted_fact(cursor)?,
            },
            3 => SelectedMemoryAccessRole::ReadByteSequence {
                index: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                length: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                obligation: decode_id(cursor, semantic_vocabulary::ObligationId::new)?,
                accepted_fact: decode_accepted_fact(cursor)?,
            },
            10 => SelectedMemoryAccessRole::WriteIndexedPrimitive {
                index: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                value: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                obligation: decode_id(cursor, semantic_vocabulary::ObligationId::new)?,
                stride: cursor.u32()?,
                extent: decode_option_u64(cursor)?,
                accepted_fact: decode_accepted_fact(cursor)?,
            },
            12 => SelectedMemoryAccessRole::ReadIndexedPrimitive {
                index: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                obligation: decode_id(cursor, semantic_vocabulary::ObligationId::new)?,
                stride: cursor.u32()?,
                extent: decode_option_u64(cursor)?,
                accepted_fact: decode_accepted_fact(cursor)?,
            },
            13 => SelectedMemoryAccessRole::WriteIndexedStructural {
                index: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                obligation: decode_id(cursor, semantic_vocabulary::ObligationId::new)?,
                stride: cursor.u32()?,
                extent: decode_option_u64(cursor)?,
                accepted_fact: decode_accepted_fact(cursor)?,
            },
            tag @ (4 | 5) => {
                let slot = decode_local_slot(cursor)?;
                if tag == 4 {
                    SelectedMemoryAccessRole::WriteLocal { slot }
                } else {
                    SelectedMemoryAccessRole::AddressLocal { slot }
                }
            }
            11 => SelectedMemoryAccessRole::ReadElementView {
                index: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                length: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                obligation: decode_id(cursor, semantic_vocabulary::ObligationId::new)?,
                accepted_fact: decode_accepted_fact(cursor)?,
            },
            14 => SelectedMemoryAccessRole::WriteElementView {
                index: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                length: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
                obligation: decode_id(cursor, semantic_vocabulary::ObligationId::new)?,
                accepted_fact: decode_accepted_fact(cursor)?,
            },
            15 => SelectedMemoryAccessRole::ReadReturnedView {
                producer: decode_id(cursor, OperationId::new)?,
            },
            16 => SelectedMemoryAccessRole::ReadMeasuredSpan {
                length: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
            },
            0 => SelectedMemoryAccessRole::ReadPlace,
            18 => SelectedMemoryAccessRole::ReadBacking,
            19 => SelectedMemoryAccessRole::WriteBacking,
            20 => SelectedMemoryAccessRole::ReadBackingSpan,
            21 => SelectedMemoryAccessRole::WriteBackingSpan,
            17 => SelectedMemoryAccessRole::ReadLocal {
                slot: decode_local_slot(cursor)?,
            },
            6 => SelectedMemoryAccessRole::WritePlace,
            1 => SelectedMemoryAccessRole::WriteOutgoing {
                slot: decode_slot(cursor)?,
            },
            2 => SelectedMemoryAccessRole::AddressOutgoing {
                slot: decode_slot(cursor)?,
            },
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        };
        function.memory_accesses.push(SelectedMemoryAccess {
            instruction,
            origin,
            place,
            byte_offset,
            byte_count,
            role,
        });
    }
    let count = cursor.length()?;
    for _ in 0..count {
        function
            .boundary_settlements
            .push(SelectedBoundarySettlement {
                block: SelectedBlockId(cursor.u32()?),
                instruction_index: cursor.u32()?,
                settlement: decode_boundary_settlement(cursor)?,
            });
    }
    Ok(())
}

pub(super) fn encode_boundary_settlement(
    bytes: &mut Vec<u8>,
    payload: &SelectedBoundarySettlementPayload,
) -> Result<(), terminal_codec::CodecError> {
    let settlement = match payload {
        SelectedBoundarySettlementPayload::HostedReadByte {
            operation,
            boundary,
            result,
            layout,
        } => {
            bytes.push(3);
            push_encoded(bytes, operation.get().to_le_bytes());
            push_encoded(bytes, boundary.get().to_le_bytes());
            encode_read_result_and_layout(bytes, result, layout);
            return Ok(());
        }
        SelectedBoundarySettlementPayload::HostedExitProcessI32 {
            edge,
            execution,
            boundary,
            source,
        } => {
            bytes.push(2);
            push_encoded(bytes, edge.get().to_le_bytes());
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
            push_encoded(bytes, boundary.get().to_le_bytes());
            push_encoded(bytes, source.get().to_le_bytes());
            return Ok(());
        }
        SelectedBoundarySettlementPayload::ClaimCompletion(settlement) => {
            bytes.push(0);
            settlement
        }
        SelectedBoundarySettlementPayload::HostedWriteByteI32 {
            operation,
            boundary,
            source,
        } => {
            bytes.push(1);
            push_encoded(bytes, operation.get().to_le_bytes());
            push_encoded(bytes, boundary.get().to_le_bytes());
            push_encoded(bytes, source.get().to_le_bytes());
            return Ok(());
        }
    };
    push_encoded(bytes, settlement.operation.get().to_le_bytes());
    push_encoded(bytes, settlement.boundary.get().to_le_bytes());
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
    encode_work(bytes, &settlement.work);
    encode_effect(bytes, settlement.effect);
    encode_ownership(bytes, &settlement.ownership);
    Ok(())
}

pub(super) fn decode_boundary_settlement(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedBoundarySettlementPayload, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        3 => {
            let operation = decode_id(cursor, OperationId::new)?;
            let boundary = decode_id(cursor, BoundaryMachineId::new)?;
            let (result, layout) = decode_read_result_and_layout(cursor)?;
            return Ok(SelectedBoundarySettlementPayload::HostedReadByte {
                operation,
                boundary,
                result,
                layout,
            });
        }
        2 => {
            return Ok(SelectedBoundarySettlementPayload::HostedExitProcessI32 {
                edge: decode_id(cursor, EdgeId::new)?,
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
            });
        }
        0 => {}
        1 => {
            return Ok(SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                operation: decode_id(cursor, OperationId::new)?,
                boundary: decode_id(cursor, BoundaryMachineId::new)?,
                source: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
            });
        }
        tag => return Err(FixedViewCopyDecodeError::UnknownBoundaryRealization(tag)),
    }
    let operation = decode_id(cursor, OperationId::new)?;
    let boundary = decode_id(cursor, BoundaryMachineId::new)?;
    let provider_execution = decode_provider_execution(cursor)?;
    match cursor.byte()? {
        1 => {}
        tag => return Err(FixedViewCopyDecodeError::UnknownBoundaryRealization(tag)),
    }
    let arguments = decode_list(cursor, decode_semantic_argument)?;
    let completion_claim_sources = decode_list(cursor, decode_completion_claim_source)?;
    let completion_receipts = decode_list(cursor, decode_completion_receipt)?;
    Ok(SelectedBoundarySettlementPayload::ClaimCompletion(
        LegalizedBoundarySettlement {
            operation,
            boundary,
            provider_execution,
            realization: ClaimCompletionOnlyRealization,
            arguments,
            completion_claim_sources,
            completion_receipts,
            work: decode_work(cursor)?,
            effect: decode_effect(cursor)?,
            ownership: decode_ownership(cursor)?,
        },
    ))
}

pub(super) fn encode_call_source(
    bytes: &mut Vec<u8>,
    source: &NativeCallOrigin,
    retain_projected_qualifications: bool,
) {
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
            length(bytes, completion_claim_sources.len());
            for source in completion_claim_sources {
                encode_completion_claim_source(bytes, source);
            }
            length(bytes, completion_receipts.len());
            for receipt in completion_receipts {
                encode_completion_receipt(bytes, receipt);
            }
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
            push_encoded(bytes, boundary.get().to_le_bytes());
            encode_provider(bytes, provider, retain_projected_qualifications);
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
        5 => {
            let boundary = decode_id(cursor, BoundaryMachineId::new)?;
            let entry = decode_id(cursor, MachineId::new)?;
            let mut source_signature = [0; 32];
            for byte in &mut source_signature {
                *byte = cursor.byte()?;
            }
            let completion_claim_sources = decode_list(cursor, decode_completion_claim_source)?;
            let completion_receipts = decode_list(cursor, decode_completion_receipt)?;
            Ok(NativeCallOrigin::ProgramEntry {
                boundary,
                entry,
                source_signature,
                completion_claim_sources,
                completion_receipts,
            })
        }
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
            let provider = decode_provider(cursor, retain_projected_qualifications)?;
            let completion_claim_sources = decode_list(cursor, decode_completion_claim_source)?;
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
    push_encoded(bytes, source.claim.get().to_le_bytes());
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
    let entry = optional(
        cursor,
        decode_entry_claim,
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    let content = optional(
        cursor,
        decode_content_entry_claim,
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    Ok(CompletionClaimSource {
        claim,
        entry,
        content,
    })
}

fn encode_content_entry_claim(bytes: &mut Vec<u8>, content: &ContentEntryClaim) {
    push_encoded(bytes, content.claim.get().to_le_bytes());
    encode_content_place(bytes, &content.input);
    length(bytes, content.projections.len());
    for projection in &content.projections {
        push_encoded(bytes, projection.projection.domain.get().to_le_bytes());
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
    let projections = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(ClaimContentProjection {
            projection: ContentProjectionIdentity {
                domain: decode_id(cursor, ContentDomainId::new)?,
                projection_report_fingerprint: cursor.u64()?,
            },
            algebra: decode_content_algebra(cursor)?,
        })
    })?;
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
    push_encoded(bytes, place.root.get().to_le_bytes());
    length(bytes, place.segments.len());
    for segment in &place.segments {
        match segment {
            ContentPlaceSegment::Field(value) => {
                bytes.push(1);
                encode_string(bytes, value);
            }
            ContentPlaceSegment::FixedIndex(value) => {
                bytes.push(2);
                push_encoded(bytes, value.to_le_bytes());
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
    let segments = decode_list(cursor, |cursor| {
        Ok(match cursor.byte()? {
            1 => ContentPlaceSegment::Field(decode_string(cursor)?),
            2 => ContentPlaceSegment::FixedIndex(cursor.u64()?),
            3 => ContentPlaceSegment::Case(decode_string(cursor)?),
            tag => return Err(FixedViewCopyDecodeError::UnknownContentPlaceSegment(tag)),
        })
    })?;
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
    push_encoded(bytes, receipt.claim.get().to_le_bytes());
    push_encoded(bytes, receipt.argument_index.to_le_bytes());
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
    push_encoded(bytes, effect.input.to_le_bytes());
    push_encoded(bytes, effect.output.to_le_bytes());
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
    let ownership = decode_list(cursor, |cursor| {
        Ok(match cursor.byte()? {
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
        })
    })?;
    Ok(ownership)
}

pub(super) fn decode_cleanup(
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
            let cleanup_receiver = optional(
                cursor,
                |cursor| decode_id(cursor, PlaceId::new),
                FixedViewCopyDecodeError::UnknownOption,
            )?;
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

pub(super) fn encode_signature(
    bytes: &mut Vec<u8>,
    value: &LegalizedStructuralContract,
) -> Result<(), terminal_codec::CodecError> {
    length(bytes, value.structural_types.len());
    for row in &value.structural_types {
        encode_type(bytes, row)?;
    }
    length(bytes, value.structural_types.referents().len());
    for referent in value.structural_types.referents() {
        push_encoded(bytes, referent.id.get().to_le_bytes());
        push_encoded(bytes, referent.identity.0.to_le_bytes());
        bytes.push(u8::from(referent.foreign_descriptor));
        bytes.push(u8::from(referent.abstract_opaque));
    }
    let quotients = terminal_codec::encode_quotient_catalog(value.structural_types.quotients())?;
    length(bytes, quotients.len());
    bytes.extend_from_slice(&quotients);
    for catalog in [
        terminal_codec::encode_access_view_catalog(value.structural_types.access_views())?,
        encode_access_view_input_catalog(value.structural_types.access_view_inputs())?,
        encode_access_lifecycle_catalog(value.structural_types.access_lifecycles())?,
    ] {
        length(bytes, catalog.len());
        bytes.extend_from_slice(&catalog);
    }
    length(bytes, value.parameters.len());
    for row in &value.parameters {
        encode_parameter(bytes, row, true);
    }
    match &value.result {
        None => bytes.push(0),
        Some(result) => {
            bytes.push(1);
            push_encoded(bytes, result.place.get().to_le_bytes());
            push_encoded(bytes, result.structural_type.get().to_le_bytes());
            encode_multiplicity(bytes, result.multiplicity);
            encode_ids(
                bytes,
                result.qualifications.iter().map(|domain| domain.get()),
            );
            encode_projected(bytes, &result.projected_qualifications, true);
            length(bytes, result.reference_sources.len());
            for reference in &result.reference_sources {
                encode_path(bytes, &reference.path);
                length(bytes, reference.sources.len());
                for source in &reference.sources {
                    match source {
                        StructuralReferenceOrigin::Parameter(source) => {
                            bytes.push(1);
                            push_encoded(bytes, source.place.get().to_le_bytes());
                        }
                        StructuralReferenceOrigin::Invocation { carrier_type, .. } => {
                            bytes.push(2);
                            push_encoded(bytes, carrier_type.get().to_le_bytes());
                        }
                    }
                    encode_reference_origin_path(bytes, source.path());
                    encode_access(bytes, source.access());
                    bytes.push(match source.extent() {
                        ReferenceOriginExtent::Projected => 1,
                        ReferenceOriginExtent::WithinRegion => 2,
                    });
                }
            }
        }
    }
    length(bytes, value.structural_places.len());
    for row in &value.structural_places {
        encode_place(bytes, *row);
    }
    length(bytes, value.entry_claims.len());
    for row in &value.entry_claims {
        encode_entry_claim(bytes, row);
    }
    encode_ids(
        bytes,
        value.published_service_ceiling.iter().map(|id| id.get()),
    );
    Ok(())
}

pub(super) fn decode_signature(
    cursor: &mut Cursor<'_>,
) -> Result<LegalizedStructuralContract, FixedViewCopyDecodeError> {
    let structural_types = decode_list(cursor, decode_type)?;
    let referents = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(terminal_psi::StructuralReferentIdentity {
            id: decode_id(cursor, StructuralTypeId::new)?,
            identity: terminal_psi::TerminalIdentityIndex(cursor.u32()?),
            foreign_descriptor: decode_bool(cursor)?,
            abstract_opaque: decode_bool(cursor)?,
        })
    })?;
    let count = cursor.length()?;
    let quotients = terminal_codec::decode_quotient_catalog(cursor.take(count)?)
        .map_err(|_| FixedViewCopyDecodeError::InvalidQuotientCatalog)?;
    terminal_semantics::quotients::validate_quotient_representations(&structural_types, &quotients)
        .map_err(|_| FixedViewCopyDecodeError::InvalidQuotientCatalog)?;
    let count = cursor.length()?;
    let access_views =
        terminal_codec::decode_access_view_catalog(cursor.take(count)?).map_err(|cause| {
            FixedViewCopyDecodeError::InvalidAccessLifecycleCatalog {
                catalog: "access views",
                cause,
            }
        })?;
    let count = cursor.length()?;
    let access_inputs = terminal_codec::decode_access_view_input_catalog(cursor.take(count)?)
        .map_err(
            |cause| FixedViewCopyDecodeError::InvalidAccessLifecycleCatalog {
                catalog: "placed inputs",
                cause,
            },
        )?;
    let count = cursor.length()?;
    let lifecycles =
        terminal_codec::decode_access_lifecycle_catalog(cursor.take(count)?).map_err(|cause| {
            FixedViewCopyDecodeError::InvalidAccessLifecycleCatalog {
                catalog: "access lifecycles",
                cause,
            }
        })?;
    let parameters = decode_list(cursor, |cursor| decode_parameter(cursor, true))?;
    let result = optional(
        cursor,
        |c| {
            Ok(terminal_psi::StructuralResultDeclaration {
                place: decode_id(c, PlaceId::new)?,
                structural_type: decode_id(c, StructuralTypeId::new)?,
                multiplicity: decode_multiplicity(c)?,
                qualifications: decode_ids(c, StructuralDomainId::new)?,
                projected_qualifications: decode_projected(c, true)?,
                reference_sources: {
                    let count = c.length()?;
                    let mut sources = Vec::new();
                    for _ in 0..count {
                        sources.push(terminal_psi::StructuralReferenceResultSource {
                            path: decode_path(c)?,
                            sources: {
                                let count = c.length()?;
                                let mut candidates = Vec::new();
                                for _ in 0..count {
                                    let (place, carrier_type) = match c.byte()? {
                                        1 => (Some(decode_id(c, PlaceId::new)?), None),
                                        2 => (None, Some(decode_id(c, StructuralTypeId::new)?)),
                                        tag => {
                                            return Err(FixedViewCopyDecodeError::UnknownOption(
                                                tag,
                                            ));
                                        }
                                    };
                                    let path = decode_reference_origin_path(c)?;
                                    let access = decode_access(c)?;
                                    let extent = match c.byte()? {
                                        1 => ReferenceOriginExtent::Projected,
                                        2 => ReferenceOriginExtent::WithinRegion,
                                        tag => return Err(
                                            FixedViewCopyDecodeError::UnknownReferenceOriginExtent(
                                                tag,
                                            ),
                                        ),
                                    };
                                    candidates.push(match (place, carrier_type) {
                                        (Some(place), None) => {
                                            StructuralReferenceOrigin::Parameter(
                                                terminal_psi::ReferenceOriginSource {
                                                    place,
                                                    path,
                                                    access,
                                                    extent,
                                                },
                                            )
                                        }
                                        (None, Some(carrier_type)) => {
                                            StructuralReferenceOrigin::Invocation {
                                                carrier_type,
                                                path,
                                                access,
                                                extent,
                                            }
                                        }
                                        _ => {
                                            return Err(FixedViewCopyDecodeError::UnknownOption(0));
                                        }
                                    });
                                }
                                candidates
                            },
                        });
                    }
                    sources
                },
            })
        },
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    let structural_places = decode_list(cursor, decode_place)?;
    let entry_claims = decode_list(cursor, decode_entry_claim)?;
    Ok(LegalizedStructuralContract {
        structural_types: StructuralTypeCatalog::new(structural_types, referents)
            .with_access_views(access_views)
            .with_access_view_inputs(access_inputs)
            .with_access_lifecycles(lifecycles)
            .map_err(
                |cause| FixedViewCopyDecodeError::InvalidAccessLifecycleCatalog {
                    catalog: "access lifecycles",
                    cause,
                },
            )?
            .with_quotients(quotients)
            .map_err(|_| FixedViewCopyDecodeError::InvalidQuotientCatalog)?,
        parameters,
        result,
        structural_places,
        entry_claims,
        published_service_ceiling: decode_ids(cursor, ServiceId::new)?,
    })
}

#[cfg(test)]
mod local_slot_tests {
    use super::{
        Cursor, FixedViewCopyDecodeError, OperationId, PlaceId, SelectedBlockId, SelectedFunction,
        SelectedInstructionId, SelectedMemoryAccess, SelectedMemoryAccessRole, decode_contracts,
        decode_local_slot, encode_contracts,
    };

    #[test]
    fn structural_call_argument_slot_roundtrip_binds_operation_and_argument_index() {
        let mut identities = std::collections::BTreeSet::new();
        for (operation, argument_index) in [(1, 0), (1, 1), (2, 0), (u64::MAX, u32::MAX)] {
            let slot = selected_instructions::LocalStorageSlotId::StructuralCallArgument {
                operation: OperationId::new(operation).unwrap(),
                argument_index,
            };
            let mut encoded = Vec::new();
            slot.encode_identity(&mut encoded);
            let mut expected = vec![5];
            super::push_encoded(&mut expected, operation.to_le_bytes());
            super::push_encoded(&mut expected, argument_index.to_le_bytes());
            assert_eq!(encoded, expected);
            assert!(identities.insert(encoded.clone()));
            let mut cursor = Cursor::new(&encoded);
            let decoded = decode_local_slot(&mut cursor).unwrap();
            assert_eq!(decoded, slot);
            assert_eq!(cursor.remaining(), 0);
            let mut reencoded = Vec::new();
            decoded.encode_identity(&mut reencoded);
            assert_eq!(encoded, reencoded);
            for truncated_length in 0..encoded.len() {
                assert!(decode_local_slot(&mut Cursor::new(&encoded[..truncated_length])).is_err());
            }
            encoded[1..9].fill(0);
            assert!(matches!(
                decode_local_slot(&mut Cursor::new(&encoded)),
                Err(FixedViewCopyDecodeError::InvalidSemanticId(0))
            ));
        }
    }

    #[test]
    fn byte_span_contract_roundtrip_retains_length_obligation_and_accepted_fact() {
        let empty = || SelectedFunction {
            machine: semantic_vocabulary::MachineId::new(1).unwrap(),
            attachment: None,
            nominal_cleanup_subject: None,
            provenance: Default::default(),
            structural: None,
            local_storage_slots: Vec::new(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            normalized_foreign_calls: Vec::new(),
            memory_accesses: Vec::new(),
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers: Vec::new(),
            blocks: (Vec::new()).into(),
        };
        let length = semantic_vocabulary::ValueId::new(7).unwrap();
        let obligation = semantic_vocabulary::ObligationId::new(8).unwrap();
        let accepted_fact = optimization_core::AcceptedObligationFactIdentity::from_bytes([9; 32]);
        let mut source = empty();
        for role in [
            SelectedMemoryAccessRole::ReadByteSpan {
                length,
                obligation,
                accepted_fact,
            },
            SelectedMemoryAccessRole::WriteByteSpan {
                length,
                obligation,
                accepted_fact,
            },
        ] {
            source.memory_accesses.push(SelectedMemoryAccess {
                instruction: SelectedInstructionId(2),
                origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
                    OperationId::new(3).unwrap(),
                ),
                place: PlaceId::new(4).unwrap(),
                byte_offset: 5,
                byte_count: 0,
                role,
            });
        }
        let mut bytes = Vec::new();
        encode_contracts(&mut bytes, &source).unwrap();
        let mut cursor = Cursor::new(&bytes);
        let mut decoded = empty();
        decode_contracts(&mut cursor, &mut decoded).unwrap();
        assert_eq!(cursor.remaining(), 0);
        assert_eq!(decoded, source);
        for mutation in 0..3 {
            let mut changed = source.clone();
            let SelectedMemoryAccessRole::WriteByteSpan {
                length,
                obligation,
                accepted_fact,
            } = &mut changed.memory_accesses[1].role
            else {
                panic!("write span");
            };
            match mutation {
                0 => *length = semantic_vocabulary::ValueId::new(17).unwrap(),
                1 => *obligation = semantic_vocabulary::ObligationId::new(18).unwrap(),
                _ => {
                    *accepted_fact =
                        optimization_core::AcceptedObligationFactIdentity::from_bytes([19; 32])
                }
            }
            let mut changed_bytes = Vec::new();
            encode_contracts(&mut changed_bytes, &changed).unwrap();
            assert_ne!(bytes, changed_bytes);
        }
        for length in 0..bytes.len() {
            assert!(decode_contracts(&mut Cursor::new(&bytes[..length]), &mut empty()).is_err());
        }
    }

    #[test]
    fn owned_entry_slot_decoder_rejects_zero_truncation_and_unknown_tag() {
        let encoded = [4, 1, 0, 0, 0, 0, 0, 0, 0];
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(
            decode_local_slot(&mut cursor).unwrap(),
            selected_instructions::LocalStorageSlotId::StructuralParameter {
                place: PlaceId::new(1).unwrap(),
            }
        );
        assert_eq!(cursor.remaining(), 0);
        for length in 0..encoded.len() {
            assert!(decode_local_slot(&mut Cursor::new(&encoded[..length])).is_err());
        }
        let mut zero_place = encoded;
        zero_place[1] = 0;
        assert!(decode_local_slot(&mut Cursor::new(&zero_place)).is_err());
        let mut unknown_tag = encoded;
        unknown_tag[0] = 255;
        assert_eq!(
            decode_local_slot(&mut Cursor::new(&unknown_tag)),
            Err(FixedViewCopyDecodeError::UnknownOption(255))
        );
    }
}

#[cfg(test)]
mod signature_tests {
    use super::{Cursor, LegalizedStructuralContract, decode_signature, encode_signature};

    #[test]
    fn recovery_signature_retains_placed_input_demand() {
        use terminal_psi::{StructuralAccess, TerminalAccessViewInput, TerminalIdentityIndex};
        let signature = LegalizedStructuralContract {
            structural_types: abstract_operations::StructuralTypeCatalog::default()
                .with_access_view_inputs(vec![TerminalAccessViewInput {
                    machine: semantic_vocabulary::MachineId::new(1).unwrap(),
                    position: 2,
                    runtime_position: 1,
                    carrier_identity: TerminalIdentityIndex(3),
                    source_machine_identity: TerminalIdentityIndex(4),
                    source_state_identity: TerminalIdentityIndex(5),
                    source_parameter_identity: TerminalIdentityIndex(6),
                    access: StructuralAccess::MutableBorrow,
                    binding_is_const: false,
                    binding_is_mutable: true,
                    view_identity: TerminalIdentityIndex(7),
                    policy_identity: TerminalIdentityIndex(8),
                    policy_plan_machine_identity: TerminalIdentityIndex(9),
                    schema_identity: TerminalIdentityIndex(10),
                    placement_report_fingerprint: 11,
                    placement_commitment: [12; 32],
                }]),
            parameters: vec![],
            result: None,
            structural_places: vec![],
            entry_claims: vec![],
            published_service_ceiling: vec![],
        };
        let mut bytes = Vec::new();
        encode_signature(&mut bytes, &signature).unwrap();
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_signature(&mut cursor).unwrap(), signature);
        assert_eq!(cursor.remaining(), 0);
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
    fn reference_signature_round_trip_retains_origins_and_referent_identity() {
        let signature = LegalizedStructuralContract {
            result: Some(terminal_psi::StructuralResultDeclaration {
                place: semantic_vocabulary::PlaceId::new(1).unwrap(),
                structural_type: semantic_vocabulary::StructuralTypeId::new(2).unwrap(),
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                reference_sources: vec![terminal_psi::StructuralReferenceResultSource {
                    path: Vec::new(),
                    sources: [2, 3]
                        .into_iter()
                        .map(|place| terminal_psi::ReferenceOriginSource {
                            place: semantic_vocabulary::PlaceId::new(place).unwrap(),
                            access: terminal_psi::StructuralAccess::MutableBorrow,
                            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                            path: Vec::new(),
                        })
                        .map(terminal_psi::StructuralReferenceOrigin::Parameter)
                        .collect(),
                }],
            }),
            structural_types: abstract_operations::StructuralTypeCatalog::new(
                vec![terminal_psi::StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: false,
                    copyable: false,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                    id: semantic_vocabulary::StructuralTypeId::new(2).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(1),
                    shape: terminal_psi::StructuralTypeShape::Reference {
                        referent: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                        access: terminal_psi::StructuralAccess::MutableBorrow,
                    },
                }],
                vec![terminal_psi::StructuralReferentIdentity {
                    id: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(2),
                    foreign_descriptor: false,
                    abstract_opaque: true,
                }],
            ),
            parameters: Vec::new(),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
        };
        let mut signature = signature;
        signature.result.as_mut().unwrap().reference_sources[0]
            .sources
            .push(terminal_psi::StructuralReferenceOrigin::Invocation {
                carrier_type: semantic_vocabulary::StructuralTypeId::new(2).unwrap(),
                path: vec![terminal_psi::ReferenceOriginProjection::Exact(
                    terminal_psi::StructuralPathSegment::Referent,
                )],
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                access: terminal_psi::StructuralAccess::MutableBorrow,
            });
        let mut bytes = Vec::new();
        encode_signature(&mut bytes, &signature).unwrap();
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_signature(&mut cursor).unwrap(), signature);
        assert_eq!(cursor.remaining(), 0);
        let mut region = signature.clone();
        parameter(&mut region.result.as_mut().unwrap().reference_sources[0].sources[0]).extent =
            semantic_vocabulary::ReferenceOriginExtent::WithinRegion;
        let mut region_bytes = Vec::new();
        encode_signature(&mut region_bytes, &region).unwrap();
        assert_ne!(region_bytes, bytes);
        let mut region_cursor = Cursor::new(&region_bytes);
        assert_eq!(decode_signature(&mut region_cursor).unwrap(), region);
        assert_eq!(region_cursor.remaining(), 0);
        let mut family = signature.clone();
        parameter(&mut family.result.as_mut().unwrap().reference_sources[0].sources[0]).path = vec![
            terminal_psi::ReferenceOriginProjection::Exact(
                terminal_psi::StructuralPathSegment::RepresentationView {
                    structural_type: semantic_vocabulary::StructuralTypeId::new(2).unwrap(),
                    stored_scalar: None,
                },
            ),
            terminal_psi::ReferenceOriginProjection::Element,
            terminal_psi::ReferenceOriginProjection::Exact(
                terminal_psi::StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(9)),
            ),
        ];
        let mut family_bytes = Vec::new();
        encode_signature(&mut family_bytes, &family).unwrap();
        assert_ne!(family_bytes, bytes);
        let mut family_cursor = Cursor::new(&family_bytes);
        assert_eq!(decode_signature(&mut family_cursor).unwrap(), family);
        assert_eq!(family_cursor.remaining(), 0);
        let mut renamed = signature.clone();
        renamed.structural_types = abstract_operations::StructuralTypeCatalog::new(
            signature.structural_types.to_vec(),
            vec![terminal_psi::StructuralReferentIdentity {
                id: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                identity: terminal_psi::TerminalIdentityIndex(3),
                foreign_descriptor: false,
                abstract_opaque: true,
            }],
        );
        let mut changed = Vec::new();
        encode_signature(&mut changed, &renamed).unwrap();
        assert_ne!(bytes, changed);
        let mut reordered = signature.clone();
        let mut descriptor = signature.clone();
        let mut referents = descriptor.structural_types.referents().to_vec();
        referents[0].foreign_descriptor = true;
        descriptor.structural_types = abstract_operations::StructuralTypeCatalog::new(
            signature.structural_types.to_vec(),
            referents,
        );
        let mut descriptor_bytes = Vec::new();
        encode_signature(&mut descriptor_bytes, &descriptor).unwrap();
        assert_ne!(bytes, descriptor_bytes);
        let mut cursor = Cursor::new(&descriptor_bytes);
        assert_eq!(decode_signature(&mut cursor).unwrap(), descriptor);
        assert_eq!(cursor.remaining(), 0);
        reordered.result.as_mut().unwrap().reference_sources[0]
            .sources
            .swap(0, 1);
        changed.clear();
        encode_signature(&mut changed, &reordered).unwrap();
        assert_ne!(bytes, changed);
        reordered.result.as_mut().unwrap().reference_sources[0]
            .sources
            .pop();
        changed.clear();
        encode_signature(&mut changed, &reordered).unwrap();
        assert_ne!(bytes, changed);
    }
}
