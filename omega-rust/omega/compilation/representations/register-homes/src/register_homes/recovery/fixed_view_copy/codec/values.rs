use optimization_unit::ValueDefinitionSite;
use register_model::{RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess};
use selected_instructions::SelectedInstructionId;
use semantic_vocabulary::{
    BlockId, IntegerCarrier, IntegerSign, IntegerType, IntegerValue, ScalarType,
};

use crate::FixedViewCopyDecodeError;
use selected_instructions::{LiveRangePoint, LivenessPosition, VirtualFixedConstraintSite};

use super::primitives::{Cursor, decode_id};

pub(super) fn decode_definition_site(
    cursor: &mut Cursor<'_>,
) -> Result<ValueDefinitionSite, FixedViewCopyDecodeError> {
    Ok(match cursor.byte()? {
        0 => ValueDefinitionSite::FunctionParameter(cursor.u32()?),
        1 => ValueDefinitionSite::BlockParameter {
            block: decode_id(cursor, BlockId::new)?,
            position: cursor.u32()?,
        },
        2 => ValueDefinitionSite::Node {
            block: decode_id(cursor, BlockId::new)?,
            node: cursor.u32()?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownDefinitionSite(tag)),
    })
}

pub(super) fn decode_fixed_site(
    cursor: &mut Cursor<'_>,
) -> Result<VirtualFixedConstraintSite, FixedViewCopyDecodeError> {
    Ok(match cursor.byte()? {
        0 => VirtualFixedConstraintSite::Entry,
        1 => VirtualFixedConstraintSite::Operand {
            position: LivenessPosition(cursor.u32()?),
            point: LiveRangePoint(cursor.u32()?),
            instruction: SelectedInstructionId(cursor.u32()?),
            operand: cursor.u16()?,
            access: decode_access(cursor)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownFixedSite(tag)),
    })
}

pub(super) fn encode_target(bytes: &mut Vec<u8>, target: target::NativeTarget) {
    crate::codec_bytes::encode_target(bytes, target);
}

pub(super) fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<target::NativeTarget, FixedViewCopyDecodeError> {
    let architecture = match cursor.byte()? {
        0 => target::Architecture::X86_64,
        1 => target::Architecture::Aarch64,
        tag => return Err(FixedViewCopyDecodeError::UnknownArchitecture(tag)),
    };
    let object_format = match cursor.byte()? {
        0 => target::ObjectFormat::Elf,
        1 => target::ObjectFormat::MachO,
        2 => target::ObjectFormat::Coff,
        tag => return Err(FixedViewCopyDecodeError::UnknownObjectFormat(tag)),
    };
    Ok(target::NativeTarget {
        architecture,
        object_format,
        pointer_size: cursor.length()?,
        pointer_alignment: cursor.length()?,
    })
}

pub(super) fn encode_scalar(bytes: &mut Vec<u8>, scalar: ScalarType) {
    match scalar {
        ScalarType::Boolean => bytes.push(0),
        ScalarType::Integer(integer) => {
            bytes.push(1);
            bytes.push(match integer.carrier() {
                IntegerCarrier::Fixed => 0,
                IntegerCarrier::Address => 1,
            });
            bytes.push(match integer.sign() {
                IntegerSign::Signed => 0,
                IntegerSign::Unsigned => 1,
            });
            crate::codec_bytes::push_encoded(bytes, integer.bits().to_le_bytes());
        }
        ScalarType::IeeeFloat(format) => {
            bytes.push(2);
            bytes.push(match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => 0,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => 1,
            });
        }
    }
}

pub(super) fn decode_scalar(
    cursor: &mut Cursor<'_>,
) -> Result<ScalarType, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(ScalarType::Boolean),
        1 => {
            let carrier = cursor.byte()?;
            let sign = match cursor.byte()? {
                0 => IntegerSign::Signed,
                1 => IntegerSign::Unsigned,
                tag => return Err(FixedViewCopyDecodeError::UnknownIntegerSign(tag)),
            };
            let bits = cursor.u16()?;
            let integer = match carrier {
                0 => IntegerType::new(sign, bits),
                1 if sign == IntegerSign::Unsigned => IntegerType::address(bits),
                tag => return Err(FixedViewCopyDecodeError::UnknownIntegerCarrier(tag)),
            }
            .map_err(|_| FixedViewCopyDecodeError::InvalidIntegerType)?;
            Ok(ScalarType::Integer(integer))
        }
        2 => match cursor.byte()? {
            0 => Ok(ScalarType::IeeeFloat(
                semantic_vocabulary::IeeeFloatFormat::Binary32,
            )),
            1 => Ok(ScalarType::IeeeFloat(
                semantic_vocabulary::IeeeFloatFormat::Binary64,
            )),
            tag => Err(FixedViewCopyDecodeError::UnknownScalarType(tag)),
        },
        tag => Err(FixedViewCopyDecodeError::UnknownScalarType(tag)),
    }
}

pub(super) fn decode_integer(
    cursor: &mut Cursor<'_>,
) -> Result<IntegerValue, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(IntegerValue::Signed(cursor.i128()?)),
        1 => Ok(IntegerValue::Unsigned(cursor.u128()?)),
        tag => Err(FixedViewCopyDecodeError::UnknownIntegerValue(tag)),
    }
}

pub(super) fn decode_constraint_key(
    cursor: &mut Cursor<'_>,
) -> Result<RegisterConstraintKey, FixedViewCopyDecodeError> {
    let family = match cursor.byte()? {
        0 => RegisterConstraintFamily::Call,
        1 => RegisterConstraintFamily::Return,
        2 => RegisterConstraintFamily::SystemCall,
        3 => RegisterConstraintFamily::InlineAssembly,
        4 => RegisterConstraintFamily::Instruction,
        tag => return Err(FixedViewCopyDecodeError::UnknownConstraintFamily(tag)),
    };
    Ok(RegisterConstraintKey {
        family,
        variant: cursor.u32()?,
    })
}

pub(super) fn decode_access(
    cursor: &mut Cursor<'_>,
) -> Result<RegisterOperandAccess, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(RegisterOperandAccess::Use),
        1 => Ok(RegisterOperandAccess::Def),
        2 => Ok(RegisterOperandAccess::UseDef),
        tag => Err(FixedViewCopyDecodeError::UnknownOperandAccess(tag)),
    }
}

pub(super) fn decode_bool(cursor: &mut Cursor<'_>) -> Result<bool, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(false),
        1 => Ok(true),
        tag => Err(FixedViewCopyDecodeError::UnknownBoolean(tag)),
    }
}
