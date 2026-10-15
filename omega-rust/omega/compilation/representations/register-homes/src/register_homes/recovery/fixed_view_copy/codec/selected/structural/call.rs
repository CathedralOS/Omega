//! Complete ordinary scalar/resultless call custody.
use super::{calling::*, declarations::*, settlements::*};
use crate::FixedViewCopyDecodeError;
use crate::register_homes::recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_accepted_fact, decode_id, decode_ids, decode_option_u64, encode_ids,
    encode_option_u64, length,
};
use legalized_operations::{
    LegalizedScalarArgument, LegalizedScalarCall, encode_call_plan, encode_placement,
};
use selected_instructions::effects::program::identity::{encode_ownership, encode_path};
use selected_instructions::{SelectedCallContract, SelectedInstructionId};
use semantic_vocabulary::{ClaimId, EdgeId, MachineId, ObligationId, OperationId, ValueId};
use terminal_psi::ClaimTransfer;
pub(super) fn encode_call(bytes: &mut Vec<u8>, row: &SelectedCallContract) {
    crate::codec_bytes::push_encoded(bytes, row.instruction.0.to_le_bytes());
    match row.owner {
        target_operations::CallSiteOwner::Operation(operation) => {
            bytes.push(0);
            crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
        }
        target_operations::CallSiteOwner::ExternalCompletion(edge) => {
            bytes.push(2);

            crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
        }
        target_operations::CallSiteOwner::CleanupAction {
            edge,
            action_ordinal,
        } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, action_ordinal.to_le_bytes());
        }
    }
    let call = &row.call;
    legalized_operations::encode_placed_call_bindings(bytes, &call.placed_bindings);
    encode_call_source(bytes, &call.source, true);
    bytes.extend_from_slice(&call.target.canonical_bytes());
    encode_call_plan(bytes, &call.call_plan);
    length(bytes, call.arguments.len());
    for argument in &call.arguments {
        match argument {
            LegalizedScalarArgument::Scalar { source, placement } => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, source.get().to_le_bytes());
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
                encode_runtime_indices(bytes, indices);
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
    length(bytes, call.descriptor_origins.len());
    for origin in &call.descriptor_origins {
        match origin {
            legalized_operations::LegalizedDescriptorOrigin::Empty => bytes.push(2),
            legalized_operations::LegalizedDescriptorOrigin::Argument {
                source,
                exact,
                indirect,
            } => {
                bytes.push(0);
                encode_semantic_argument(bytes, source);
                bytes.push(u8::from(*exact));
                bytes.push(u8::from(*indirect));
            }
            legalized_operations::LegalizedDescriptorOrigin::Literal { machine, place } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, machine.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            }
        }
    }
    length(bytes, call.claim_transfers.len());
    for transfer in &call.claim_transfers {
        crate::codec_bytes::push_encoded(bytes, transfer.claim.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, transfer.argument_index.to_le_bytes());
        encode_path(bytes, &transfer.path);
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
        2 => target_operations::CallSiteOwner::ExternalCompletion(decode_id(
            cursor,
            semantic_vocabulary::EdgeId::new,
        )?),
        1 => target_operations::CallSiteOwner::CleanupAction {
            edge: decode_id(cursor, EdgeId::new)?,
            action_ordinal: cursor.u32()?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
    let placed_bindings = super::placed_bindings::decode(cursor)?;
    let source = decode_call_source(cursor, true)?;
    let target = match cursor.byte()? {
        0 => target_operations::NativeCallTarget::Machine(decode_id(cursor, MachineId::new)?),
        1 => target_operations::NativeCallTarget::Dynamic {
            selector: decode_id(cursor, ValueId::new)?,
            interface: cursor.u32()?,
            slot: cursor.u32()?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    };
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
                let indices = decode_runtime_indices(cursor)?;
                LegalizedScalarArgument::Structural {
                    semantic,
                    target,
                    indices,
                }
            }
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        });
    }
    let result_placement = crate::cursor::optional(
        cursor,
        |c| Ok(decode_placement(c)?),
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    let structural_result = crate::cursor::optional(
        cursor,
        |c| Ok(super::read_result::decode_result(c)?),
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    let count = cursor.length()?;
    let mut descriptor_origins = Vec::new();
    for _ in 0..count {
        descriptor_origins.push(match cursor.byte()? {
            0 => {
                let source = decode_semantic_argument(cursor)?;
                let exact = match cursor.byte()? {
                    0 => false,
                    1 => true,
                    tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
                };
                let indirect = match cursor.byte()? {
                    0 => false,
                    1 => true,
                    tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
                };
                legalized_operations::LegalizedDescriptorOrigin::Argument {
                    source,
                    exact,
                    indirect,
                }
            }
            1 => legalized_operations::LegalizedDescriptorOrigin::Literal {
                machine: decode_id(cursor, MachineId::new)?,
                place: decode_id(cursor, semantic_vocabulary::PlaceId::new)?,
            },
            2 => legalized_operations::LegalizedDescriptorOrigin::Empty,
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        });
    }
    let count = cursor.length()?;
    let mut claim_transfers = Vec::new();
    for _ in 0..count {
        claim_transfers.push(ClaimTransfer {
            claim: decode_id(cursor, ClaimId::new)?,
            argument_index: cursor.u32()?,
            path: decode_path(cursor)?,
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
            descriptor_origins,
            placed_bindings,
            source,
            target,
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

/// Ordinary and normalized foreign calls retain the same evaluated selector
/// custody. The extent and accepted fact are part of each operand's bytes.
pub(in super::super) fn encode_runtime_indices(
    bytes: &mut Vec<u8>,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
) {
    length(bytes, indices.len());
    for index in indices {
        crate::codec_bytes::push_encoded(bytes, index.operand.value.get().to_le_bytes());
        crate::register_homes::recovery::fixed_view_copy::codec::values::encode_scalar(
            bytes,
            index.operand.scalar_type,
        );
        crate::codec_bytes::push_encoded(bytes, index.stride.to_le_bytes());
        encode_option_u64(bytes, index.extent);
        crate::codec_bytes::push_encoded(bytes, index.obligation.get().to_le_bytes());
        bytes.extend_from_slice(&index.accepted_fact.bytes());
    }
}

pub(in super::super) fn decode_runtime_indices(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<legalized_operations::LegalizedRuntimeIndexOperand>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut indices = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        indices.push(legalized_operations::LegalizedRuntimeIndexOperand {
            operand: abstract_operations::AbstractResult {
                value: decode_id(cursor, ValueId::new)?,
                scalar_type:
                    crate::register_homes::recovery::fixed_view_copy::codec::values::decode_scalar(
                        cursor,
                    )?,
            },
            stride: cursor.u32()?,
            extent: decode_option_u64(cursor)?,
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: decode_accepted_fact(cursor)?,
        });
    }
    Ok(indices)
}
