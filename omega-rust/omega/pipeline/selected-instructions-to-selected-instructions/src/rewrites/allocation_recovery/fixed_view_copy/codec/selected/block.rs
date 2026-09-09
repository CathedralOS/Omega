use abstract_operations::ValueBinding;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedSuccessor, SelectedSuccessorRole,
    SelectedTerminator, SelectedValueBinding, SelectedValueTransport, VirtualRegisterId,
};
use semantic_vocabulary::{BlockId, EdgeId, ValueId};

#[cfg(test)]
mod tests;

use crate::FixedViewCopyDecodeError;

use super::{
    instruction::{decode_instruction, encode_instruction},
    provenance::{decode_fuel, encode_fuel},
};
use crate::rewrites::allocation_recovery::fixed_view_copy::codec::{
    primitives::{Cursor, decode_id, length},
    values::{decode_scalar, encode_scalar},
};

pub(super) fn encode_block(bytes: &mut Vec<u8>, block: &SelectedBlock) {
    bytes.extend_from_slice(&block.id.0.to_le_bytes());
    match block.origin {
        SelectedBlockOrigin::Source(source) => {
            bytes.push(0);
            bytes.extend_from_slice(&source.get().to_le_bytes());
        }
        SelectedBlockOrigin::CaseDispatch { source, case_ordinal } => {
            bytes.push(2);
            bytes.extend_from_slice(&source.get().to_le_bytes());
            bytes.extend_from_slice(&case_ordinal.to_le_bytes());
        }
        SelectedBlockOrigin::EdgeTransfer { edge, target } => {
            bytes.push(1);
            bytes.extend_from_slice(&edge.get().to_le_bytes());
            bytes.extend_from_slice(&target.get().to_le_bytes());
        }
    }
    length(bytes, block.instructions.len());
    for instruction in &block.instructions {
        encode_instruction(bytes, instruction);
    }
    match &block.terminator {
        SelectedTerminator::HostedExitProcess {
            instruction,
            nominal_return_edge,
        } => {
            bytes.push(5);
            encode_instruction(bytes, instruction);
            bytes.extend_from_slice(&nominal_return_edge.get().to_le_bytes());
        }
        SelectedTerminator::Jump {
            instruction,
            successor,
        } => {
            bytes.push(4);
            encode_instruction(bytes, instruction);
            encode_successor(bytes, successor);
        }
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => {
            bytes.push(0);
            encode_instruction(bytes, instruction);
            encode_successor(bytes, when_nonzero);
            encode_successor(bytes, when_zero);
        }
        SelectedTerminator::Return {
            instruction,
            psi_return_edge,
        } => {
            bytes.push(1);
            encode_instruction(bytes, instruction);
            bytes.extend_from_slice(&psi_return_edge.get().to_le_bytes());
        }
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => {
            bytes.push(2);
            encode_instruction(bytes, instruction);
            encode_successor(bytes, when_less);
            encode_successor(bytes, when_not_less);
        }
        SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => {
            bytes.push(3);
            encode_instruction(bytes, instruction);
            encode_successor(bytes, when_less);
            encode_successor(bytes, when_not_less);
        }
    }
}

pub(super) fn decode_block(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedBlock, FixedViewCopyDecodeError> {
    let id = SelectedBlockId(cursor.u32()?);
    let origin = match cursor.byte()? {
        0 => SelectedBlockOrigin::Source(decode_id(cursor, BlockId::new)?),
        1 => SelectedBlockOrigin::EdgeTransfer {
            edge: decode_id(cursor, EdgeId::new)?,
            target: decode_id(cursor, BlockId::new)?,
        },
        2 => SelectedBlockOrigin::CaseDispatch {
            source: decode_id(cursor, BlockId::new)?, case_ordinal: cursor.u32()?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownBlockOrigin(tag)),
    };
    let instruction_count = cursor.length()?;
    let mut instructions = Vec::with_capacity(instruction_count.min(cursor.remaining()));
    for _ in 0..instruction_count {
        instructions.push(decode_instruction(cursor)?);
    }
    let terminator = match cursor.byte()? {
        5 => SelectedTerminator::HostedExitProcess {
            instruction: decode_instruction(cursor)?,
            nominal_return_edge: decode_id(cursor, EdgeId::new)?,
        },
        4 => SelectedTerminator::Jump {
            instruction: decode_instruction(cursor)?,
            successor: decode_successor(cursor)?,
        },
        0 => SelectedTerminator::ConditionalBranch {
            instruction: decode_instruction(cursor)?,
            when_nonzero: decode_successor(cursor)?,
            when_zero: decode_successor(cursor)?,
        },
        1 => SelectedTerminator::Return {
            instruction: decode_instruction(cursor)?,
            psi_return_edge: decode_id(cursor, EdgeId::new)?,
        },
        2 => SelectedTerminator::ConditionalBranchU64LessThan {
            instruction: decode_instruction(cursor)?,
            when_less: decode_successor(cursor)?,
            when_not_less: decode_successor(cursor)?,
        },
        3 => SelectedTerminator::ConditionalBranchI64LessThan {
            instruction: decode_instruction(cursor)?,
            when_less: decode_successor(cursor)?,
            when_not_less: decode_successor(cursor)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownTerminator(tag)),
    };
    Ok(SelectedBlock {
        id,
        origin,
        instructions,
        terminator,
    })
}

fn encode_successor(bytes: &mut Vec<u8>, successor: &SelectedSuccessor) {
    bytes.push(match successor.role {
        SelectedSuccessorRole::Semantic => 0,
        SelectedSuccessorRole::EdgeTransferContinuation => 1,
        SelectedSuccessorRole::CaseDispatchContinuation => 2,
    });
    bytes.extend_from_slice(&successor.psi_edge.get().to_le_bytes());
    bytes.extend_from_slice(&successor.block.0.to_le_bytes());
    bytes.extend_from_slice(&successor.source_target.get().to_le_bytes());
    length(bytes, successor.bindings.len());
    for binding in &successor.bindings {
        bytes.extend_from_slice(&binding.semantic.parameter.get().to_le_bytes());
        bytes.extend_from_slice(&binding.semantic.argument.get().to_le_bytes());
        encode_scalar(bytes, binding.semantic.scalar_type);
        match binding.transport {
            SelectedValueTransport::Unused => bytes.push(0),
            SelectedValueTransport::Registers {
                argument,
                parameter,
            } => {
                bytes.push(1);
                bytes.extend_from_slice(&argument.0.to_le_bytes());
                bytes.extend_from_slice(&parameter.0.to_le_bytes());
            }
        }
    }
    match &successor.structural_case {
        None => bytes.push(0),
        Some(case) => {
            bytes.push(1);
            case.encode_identity(bytes);
        }
    }
    length(bytes, successor.structural_bindings.len());
    for binding in &successor.structural_bindings {
        bytes.extend_from_slice(&binding.semantic.parameter.get().to_le_bytes());
        super::structural::encode_semantic_argument(bytes, &binding.semantic.argument);
        match binding.transport {
            selected_instructions::SelectedStructuralTransport::Unused => bytes.push(0),
            selected_instructions::SelectedStructuralTransport::Descriptor {
                argument,
                destination,
            } => {
                bytes.push(1);
                bytes.extend_from_slice(&argument.0.to_le_bytes());
                destination.encode_identity(bytes);
            }
        }
    }
    encode_fuel(bytes, &successor.fuel);
}

fn decode_successor(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedSuccessor, FixedViewCopyDecodeError> {
    let role = match cursor.byte()? {
        0 => SelectedSuccessorRole::Semantic,
        1 => SelectedSuccessorRole::EdgeTransferContinuation,
        2 => SelectedSuccessorRole::CaseDispatchContinuation,
        tag => return Err(FixedViewCopyDecodeError::UnknownSuccessorRole(tag)),
    };
    let psi_edge = decode_id(cursor, EdgeId::new)?;
    let block = SelectedBlockId(cursor.u32()?);
    let source_target = decode_id(cursor, BlockId::new)?;
    let count = cursor.length()?;
    let mut bindings = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        let semantic = ValueBinding {
            parameter: decode_id(cursor, ValueId::new)?,
            argument: decode_id(cursor, ValueId::new)?,
            scalar_type: decode_scalar(cursor)?,
        };
        let transport = match cursor.byte()? {
            0 => SelectedValueTransport::Unused,
            1 => SelectedValueTransport::Registers {
                argument: VirtualRegisterId(cursor.u32()?),
                parameter: VirtualRegisterId(cursor.u32()?),
            },
            tag => return Err(FixedViewCopyDecodeError::UnknownValueTransport(tag)),
        };
        bindings.push(SelectedValueBinding {
            semantic,
            transport,
        });
    }
    let structural_case = decode_case(cursor)?;
    let count = cursor.length()?;
    let mut structural_bindings = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        let semantic = abstract_operations::AbstractStructuralBinding {
            parameter: decode_id(cursor, semantic_vocabulary::PlaceId::new)?,
            argument: super::structural::decode_semantic_argument(cursor)?,
        };
        let transport = match cursor.byte()? {
            0 => selected_instructions::SelectedStructuralTransport::Unused,
            1 => selected_instructions::SelectedStructuralTransport::Descriptor {
                argument: VirtualRegisterId(cursor.u32()?),
                destination: super::structural::decode_local_slot(cursor)?,
            },
            tag => return Err(FixedViewCopyDecodeError::UnknownValueTransport(tag)),
        };
        structural_bindings.push(selected_instructions::SelectedStructuralBinding {
            semantic,
            transport,
        });
    }
    Ok(SelectedSuccessor {
        role,
        psi_edge,
        block,
        source_target,
        bindings,
        structural_bindings,
        structural_case,
        fuel: decode_fuel(cursor)?,
    })
}

fn decode_case(
    cursor: &mut Cursor<'_>,
) -> Result<Option<selected_instructions::SelectedStructuralCaseEdge>, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => return Ok(None),
        1 => {}
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    }
    let slot = super::structural::decode_local_slot(cursor)?;
    let case = decode_id(cursor, semantic_vocabulary::StructuralCaseId::new)?;
    let case_tag = i32::from_le_bytes(cursor.array()?);
    let count = cursor.length()?;
    let mut payloads = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        let field = decode_id(cursor, semantic_vocabulary::StructuralFieldId::new)?;
        let field_byte_offset = cursor.u32()?;
        let parameter = legalized_operations::LegalizedValueDefinition {
            value: decode_id(cursor, ValueId::new)?,
            scalar_type: decode_scalar(cursor)?,
            definition_site: crate::rewrites::allocation_recovery::fixed_view_copy::codec::values::decode_definition_site(cursor)?,
        };
        let transport = match cursor.byte()? {
            0 => selected_instructions::SelectedCasePayloadTransport::Unused,
            1 => selected_instructions::SelectedCasePayloadTransport::Unmaterialized {
                parameter: VirtualRegisterId(cursor.u32()?),
            },
            2 => selected_instructions::SelectedCasePayloadTransport::Registers {
                argument: VirtualRegisterId(cursor.u32()?),
                parameter: VirtualRegisterId(cursor.u32()?),
            },
            tag => return Err(FixedViewCopyDecodeError::UnknownValueTransport(tag)),
        };
        payloads.push(selected_instructions::SelectedCasePayloadBinding {
            semantic: legalized_operations::LegalizedStructuralCasePayload {
                field,
                field_byte_offset,
                parameter,
            },
            transport,
        });
    }
    let count = cursor.length()?;
    let mut trivial_affine_discards = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        trivial_affine_discards.push(decode_id(cursor, semantic_vocabulary::PlaceId::new)?);
    }
    Ok(Some(selected_instructions::SelectedStructuralCaseEdge {
        slot,
        case,
        case_tag,
        payloads,
        trivial_affine_discards,
    }))
}
