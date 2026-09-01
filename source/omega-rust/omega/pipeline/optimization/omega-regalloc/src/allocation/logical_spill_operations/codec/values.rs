use omega_optimization_unit::ValueDefinitionSite;
use omega_selected_instructions::{SelectedInstructionId, VirtualRegisterOrigin};
use psi_core::{BlockId, IntegerCarrier, IntegerSign, IntegerType, ScalarType, ValueId};

use super::super::LogicalSpillOperationDecodeError;
use super::cursor::Cursor;

pub(super) fn encode_len(bytes: &mut Vec<u8>, length: usize) {
    bytes.extend_from_slice(
        &u64::try_from(length)
            .expect("logical-spill canonical length fits u64")
            .to_le_bytes(),
    );
}

pub(super) fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: ScalarType) {
    match scalar_type {
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
            bytes.extend_from_slice(&integer.bits().to_le_bytes());
        }
    }
}

pub(super) fn decode_scalar_type(
    cursor: &mut Cursor<'_>,
) -> Result<ScalarType, LogicalSpillOperationDecodeError> {
    match cursor.byte()? {
        0 => Ok(ScalarType::Boolean),
        1 => {
            let carrier = match cursor.byte()? {
                0 => IntegerCarrier::Fixed,
                1 => IntegerCarrier::Address,
                tag => return Err(LogicalSpillOperationDecodeError::UnknownIntegerCarrier(tag)),
            };
            let sign = match cursor.byte()? {
                0 => IntegerSign::Signed,
                1 => IntegerSign::Unsigned,
                tag => return Err(LogicalSpillOperationDecodeError::UnknownIntegerSign(tag)),
            };
            let bits = u16::from_le_bytes(cursor.array()?);
            let integer = match carrier {
                IntegerCarrier::Fixed => IntegerType::new(sign, bits),
                IntegerCarrier::Address if sign == IntegerSign::Unsigned => {
                    IntegerType::address(bits)
                }
                IntegerCarrier::Address => {
                    return Err(LogicalSpillOperationDecodeError::InvalidIntegerType);
                }
            }
            .map_err(|_| LogicalSpillOperationDecodeError::InvalidIntegerType)?;
            Ok(ScalarType::Integer(integer))
        }
        tag => Err(LogicalSpillOperationDecodeError::UnknownScalarType(tag)),
    }
}

pub(super) fn encode_origin(bytes: &mut Vec<u8>, origin: VirtualRegisterOrigin) {
    match origin {
        VirtualRegisterOrigin::EntryParameter {
            source_value,
            parameter_index,
        } => {
            bytes.push(0);
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
            encode_len(bytes, parameter_index);
        }
        VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
        }
        VirtualRegisterOrigin::LegalizationTemporary {
            instruction,
            temporary,
            source_value,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&temporary.0.to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
        }
    }
}

pub(super) fn decode_origin(
    cursor: &mut Cursor<'_>,
) -> Result<VirtualRegisterOrigin, LogicalSpillOperationDecodeError> {
    let value =
        |raw| ValueId::new(raw).ok_or(LogicalSpillOperationDecodeError::InvalidValueId(raw));
    match cursor.byte()? {
        0 => {
            let raw = u64::from_le_bytes(cursor.array()?);
            Ok(VirtualRegisterOrigin::EntryParameter {
                source_value: value(raw)?,
                parameter_index: cursor.length()?,
            })
        }
        1 => {
            let instruction = SelectedInstructionId(u32::from_le_bytes(cursor.array()?));
            let raw = u64::from_le_bytes(cursor.array()?);
            Ok(VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value: value(raw)?,
            })
        }
        2 => {
            let instruction = SelectedInstructionId(u32::from_le_bytes(cursor.array()?));
            let temporary = omega_legalized_operations::LegalizedTemporaryId(u32::from_le_bytes(
                cursor.array()?,
            ));
            let raw = u64::from_le_bytes(cursor.array()?);
            Ok(VirtualRegisterOrigin::LegalizationTemporary {
                instruction,
                temporary,
                source_value: value(raw)?,
            })
        }
        tag => Err(LogicalSpillOperationDecodeError::UnknownOrigin(tag)),
    }
}

pub(super) fn encode_definition_site(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
    match site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(0);
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(1);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&node.to_le_bytes());
        }
    }
}

pub(super) fn decode_definition_site(
    cursor: &mut Cursor<'_>,
) -> Result<ValueDefinitionSite, LogicalSpillOperationDecodeError> {
    let block =
        |raw| BlockId::new(raw).ok_or(LogicalSpillOperationDecodeError::InvalidBlockId(raw));
    match cursor.byte()? {
        0 => Ok(ValueDefinitionSite::FunctionParameter(u32::from_le_bytes(
            cursor.array()?,
        ))),
        1 => {
            let raw = u64::from_le_bytes(cursor.array()?);
            Ok(ValueDefinitionSite::BlockParameter {
                block: block(raw)?,
                position: u32::from_le_bytes(cursor.array()?),
            })
        }
        2 => {
            let raw = u64::from_le_bytes(cursor.array()?);
            Ok(ValueDefinitionSite::Node {
                block: block(raw)?,
                node: u32::from_le_bytes(cursor.array()?),
            })
        }
        tag => Err(LogicalSpillOperationDecodeError::UnknownDefinitionSite(tag)),
    }
}
