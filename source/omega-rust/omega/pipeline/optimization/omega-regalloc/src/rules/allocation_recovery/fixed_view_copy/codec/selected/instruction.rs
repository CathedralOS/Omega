use omega_register_model::{RegisterClassId, RegisterUnitId, RegisterViewId};
use omega_selected_instructions::{
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedOperand,
    VirtualRegisterId,
};
use psi_core::{MachineId, ObligationId};

use crate::FixedViewCopyDecodeError;

use super::provenance::{decode_provenance, encode_provenance};
use crate::rules::allocation_recovery::fixed_view_copy::codec::{
    primitives::{
        Cursor, decode_id, decode_option_u16, decode_u16s, encode_option_u16, encode_u16s, length,
    },
    values::{
        access_tag, decode_access, decode_bool, decode_constraint_key, decode_integer,
        encode_constraint_key, encode_integer,
    },
};

pub(super) fn encode_instruction(bytes: &mut Vec<u8>, instruction: &SelectedInstruction) {
    bytes.extend_from_slice(&instruction.id.0.to_le_bytes());
    encode_kind(bytes, instruction.kind);
    encode_constraint_key(bytes, instruction.constraint);
    length(bytes, instruction.operands.len());
    for operand in &instruction.operands {
        bytes.extend_from_slice(&operand.operand.to_le_bytes());
        bytes.extend_from_slice(&operand.virtual_register.0.to_le_bytes());
        bytes.push(access_tag(operand.access));
        bytes.extend_from_slice(&operand.class.0.to_le_bytes());
        encode_option_u16(bytes, operand.fixed_view.map(|view| view.0));
        encode_option_u16(bytes, operand.tied_to);
        bytes.push(u8::from(operand.early_clobber));
    }
    encode_u16s(bytes, instruction.implicit_uses.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.implicit_defs.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.clobbers.iter().map(|unit| unit.0));
    encode_provenance(bytes, &instruction.provenance);
}

pub(super) fn decode_instruction(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstruction, FixedViewCopyDecodeError> {
    let id = SelectedInstructionId(cursor.u32()?);
    let kind = decode_kind(cursor)?;
    let constraint = decode_constraint_key(cursor)?;
    let operand_count = cursor.length()?;
    let mut operands = Vec::with_capacity(operand_count.min(cursor.remaining()));
    for _ in 0..operand_count {
        operands.push(SelectedOperand {
            operand: cursor.u16()?,
            virtual_register: VirtualRegisterId(cursor.u32()?),
            access: decode_access(cursor)?,
            class: RegisterClassId(cursor.u16()?),
            fixed_view: decode_option_u16(cursor)?.map(RegisterViewId),
            tied_to: decode_option_u16(cursor)?,
            early_clobber: decode_bool(cursor)?,
        });
    }
    Ok(SelectedInstruction {
        id,
        kind,
        constraint,
        operands,
        implicit_uses: decode_u16s(cursor)?
            .into_iter()
            .map(RegisterUnitId)
            .collect(),
        implicit_defs: decode_u16s(cursor)?
            .into_iter()
            .map(RegisterUnitId)
            .collect(),
        clobbers: decode_u16s(cursor)?
            .into_iter()
            .map(RegisterUnitId)
            .collect(),
        provenance: decode_provenance(cursor)?,
    })
}

fn encode_kind(bytes: &mut Vec<u8>, kind: SelectedInstructionKind) {
    let tag = match kind {
        SelectedInstructionKind::CompareI64Zero => 0,
        SelectedInstructionKind::MaterializeI64 { .. } => 1,
        SelectedInstructionKind::ConditionalBranchNonZero => 2,
        SelectedInstructionKind::ReturnI64 => 3,
        SelectedInstructionKind::CopyI64 => 4,
        SelectedInstructionKind::ExactAddI64 { .. } => 5,
        SelectedInstructionKind::ExactAddI64Immediate { .. } => 6,
        SelectedInstructionKind::ExactSubtractI64 { .. } => 7,
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => 8,
        SelectedInstructionKind::ReturnUnit => 9,
        SelectedInstructionKind::CompareI64 => 10,
        SelectedInstructionKind::ConditionalBranchU64LessThan => 11,
        SelectedInstructionKind::CallI64 { .. } => 12,
    };
    bytes.push(tag);
    match kind {
        SelectedInstructionKind::MaterializeI64 { value } => encode_integer(bytes, value),
        SelectedInstructionKind::ExactAddI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactSubtractI64 {
            obligation,
            accepted_fact,
        } => {
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::ExactAddI64Immediate {
            immediate,
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactSubtractI64Immediate {
            immediate,
            obligation,
            accepted_fact,
        } => {
            encode_integer(bytes, immediate);
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::CallI64 { callee } => {
            bytes.extend_from_slice(&callee.get().to_le_bytes());
        }
        _ => {}
    }
}

pub(in crate::rules::allocation_recovery::fixed_view_copy::codec) fn decode_kind(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionKind, FixedViewCopyDecodeError> {
    Ok(match cursor.byte()? {
        0 => SelectedInstructionKind::CompareI64Zero,
        1 => SelectedInstructionKind::MaterializeI64 {
            value: decode_integer(cursor)?,
        },
        2 => SelectedInstructionKind::ConditionalBranchNonZero,
        3 => SelectedInstructionKind::ReturnI64,
        4 => SelectedInstructionKind::CopyI64,
        5 => SelectedInstructionKind::ExactAddI64 {
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: omega_optimization_core::AcceptedObligationFactIdentity::from_bytes(
                cursor.array()?,
            ),
        },
        6 => SelectedInstructionKind::ExactAddI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: omega_optimization_core::AcceptedObligationFactIdentity::from_bytes(
                cursor.array()?,
            ),
        },
        7 => SelectedInstructionKind::ExactSubtractI64 {
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: omega_optimization_core::AcceptedObligationFactIdentity::from_bytes(
                cursor.array()?,
            ),
        },
        8 => SelectedInstructionKind::ExactSubtractI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: omega_optimization_core::AcceptedObligationFactIdentity::from_bytes(
                cursor.array()?,
            ),
        },
        9 => SelectedInstructionKind::ReturnUnit,
        10 => SelectedInstructionKind::CompareI64,
        11 => SelectedInstructionKind::ConditionalBranchU64LessThan,
        12 => SelectedInstructionKind::CallI64 {
            callee: decode_id(cursor, MachineId::new)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownInstructionKind(tag)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_i64_uses_append_only_tag_ten() {
        let mut bytes = Vec::new();
        encode_kind(&mut bytes, SelectedInstructionKind::CompareI64);
        assert_eq!(bytes, [10]);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(
            decode_kind(&mut cursor).unwrap(),
            SelectedInstructionKind::CompareI64
        );
    }

    #[test]
    fn conditional_branch_u64_less_than_uses_append_only_tag_eleven() {
        let mut bytes = Vec::new();
        encode_kind(
            &mut bytes,
            SelectedInstructionKind::ConditionalBranchU64LessThan,
        );
        assert_eq!(bytes, [11]);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(
            decode_kind(&mut cursor).unwrap(),
            SelectedInstructionKind::ConditionalBranchU64LessThan
        );
    }

    #[test]
    fn call_i64_uses_append_only_tag_twelve_and_binds_callee() {
        let callee = MachineId::new(47).unwrap();
        let mut bytes = Vec::new();
        encode_kind(&mut bytes, SelectedInstructionKind::CallI64 { callee });
        assert_eq!(bytes[0], 12);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(
            decode_kind(&mut cursor).unwrap(),
            SelectedInstructionKind::CallI64 { callee }
        );
    }
}
