//! Complete ordinary scalar/resultless call custody.
use super::{calling::*, declarations::*, settlements::*};
use crate::FixedViewCopyDecodeError;
use crate::register_homes::recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_id, decode_ids, decode_option_u64, encode_ids, encode_option_u64, length,
};
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarCall};
use selected_instructions::{SelectedCallContract, SelectedInstructionId};
use semantic_vocabulary::{ClaimId, EdgeId, MachineId, ObligationId, OperationId, ValueId};
use terminal_psi::ClaimTransfer;
pub(super) fn encode_call(bytes: &mut Vec<u8>, row: &SelectedCallContract) {
    bytes.extend_from_slice(&row.instruction.0.to_le_bytes());
    match row.owner {
        target_operations::CallSiteOwner::Operation(operation) => {
            bytes.push(0);
            bytes.extend_from_slice(&operation.get().to_le_bytes());
        }
        target_operations::CallSiteOwner::CleanupAction {
            edge,
            action_ordinal,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&edge.get().to_le_bytes());
            bytes.extend_from_slice(&action_ordinal.to_le_bytes());
        }
    }
    let call = &row.call;
    encode_call_source(bytes, &call.source, true);
    bytes.extend_from_slice(&call.callee.get().to_le_bytes());
    encode_call_plan(bytes, &call.call_plan);
    length(bytes, call.arguments.len());
    for argument in &call.arguments {
        match argument {
            LegalizedScalarArgument::Scalar { source, placement } => {
                bytes.push(0);
                bytes.extend_from_slice(&source.get().to_le_bytes());
                encode_placement(bytes, placement);
            }
            LegalizedScalarArgument::Structural {
                semantic,
                target,
                indices,
            } => {
                bytes.push(1);
                encode_semantic_argument(bytes, semantic);
                encode_target_argument(bytes, target);
                length(bytes, indices.len());
                for index in indices {
                    bytes.extend_from_slice(&index.operand.value.get().to_le_bytes());
                    crate::register_homes::recovery::fixed_view_copy::codec::values::encode_scalar(
                        bytes,
                        index.operand.scalar_type,
                    );
                    bytes.extend_from_slice(&index.stride.to_le_bytes());
                    encode_option_u64(bytes, index.extent);
                    bytes.extend_from_slice(&index.obligation.get().to_le_bytes());
                    bytes.extend_from_slice(&index.accepted_fact.bytes());
                }
            }
        }
    }
    match &call.result_placement {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            encode_placement(bytes, value);
        }
    }
    match &call.structural_result {
        None => bytes.push(0),
        Some(result) => {
            bytes.push(1);
            super::read_result::encode_result(bytes, result);
        }
    }
    length(bytes, call.claim_transfers.len());
    for transfer in &call.claim_transfers {
        bytes.extend_from_slice(&transfer.claim.get().to_le_bytes());
        bytes.extend_from_slice(&transfer.argument_index.to_le_bytes());
    }
    encode_ids(
        bytes,
        call.requirement_obligations.iter().map(|id| id.get()),
    );
    let crash = terminal_codec::encode_crash_route_buckets(&call.crash_continuations)
        .expect("canonical call crash routes");
    length(bytes, crash.len());
    bytes.extend_from_slice(&crash);
    encode_effect(bytes, row.effect);
    encode_ownership(bytes, &row.ownership);
}
pub(super) fn decode_call(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedCallContract, FixedViewCopyDecodeError> {
    let instruction = SelectedInstructionId(cursor.u32()?);
    let owner = match cursor.byte()? {
        0 => target_operations::CallSiteOwner::Operation(decode_id(cursor, OperationId::new)?),
        1 => target_operations::CallSiteOwner::CleanupAction {
            edge: decode_id(cursor, EdgeId::new)?,
            action_ordinal: cursor.u32()?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    let source = decode_call_source(cursor, true)?;
    let callee = decode_id(cursor, MachineId::new)?;
    let call_plan = decode_call_plan(cursor)?;
    let count = cursor.length()?;
    let mut arguments = Vec::new();
    for _ in 0..count {
        arguments.push(match cursor.byte()? {
            0 => LegalizedScalarArgument::Scalar {
                source: decode_id(cursor, ValueId::new)?,
                placement: decode_placement(cursor)?,
            },
            1 => {
                let semantic = decode_semantic_argument(cursor)?;
                let target = decode_target_argument(cursor)?;
                let count = cursor.length()?;
                let mut indices = Vec::new();
                for _ in 0..count {
                    indices.push(
                        legalized_operations::LegalizedRuntimeIndexOperand {
                            operand: abstract_operations::AbstractResult {
                                value: decode_id(cursor, ValueId::new)?,
                                scalar_type: crate::register_homes::recovery::fixed_view_copy::codec::values::decode_scalar(cursor)?,
                            },
                            stride: cursor.u32()?,
                            extent: decode_option_u64(cursor)?,
                            obligation: decode_id(cursor, ObligationId::new)?,
                            accepted_fact:
                                optimization_core::AcceptedObligationFactIdentity::from_bytes(
                                    cursor.array()?,
                                ),
                        },
                    );
                }
                LegalizedScalarArgument::Structural {
                    semantic,
                    target,
                    indices,
                }
            }
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        });
    }
    let result_placement = match cursor.byte()? {
        0 => None,
        1 => Some(decode_placement(cursor)?),
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    let structural_result = match cursor.byte()? {
        0 => None,
        1 => Some(super::read_result::decode_result(cursor)?),
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    let count = cursor.length()?;
    let mut claim_transfers = Vec::new();
    for _ in 0..count {
        claim_transfers.push(ClaimTransfer {
            claim: decode_id(cursor, ClaimId::new)?,
            argument_index: cursor.u32()?,
        });
    }
    let requirement_obligations = decode_ids(cursor, ObligationId::new)?;
    let length = cursor.length()?;
    let crash_continuations = terminal_codec::decode_crash_route_buckets(cursor.take(length)?)
        .map_err(|_| FixedViewCopyDecodeError::InvalidCrashContinuations)?;
    Ok(SelectedCallContract {
        instruction,
        owner,
        call: LegalizedScalarCall {
            source,
            callee,
            call_plan,
            arguments,
            result_placement,
            structural_result,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        },
        effect: decode_effect(cursor)?,
        ownership: decode_ownership(cursor)?,
    })
}
