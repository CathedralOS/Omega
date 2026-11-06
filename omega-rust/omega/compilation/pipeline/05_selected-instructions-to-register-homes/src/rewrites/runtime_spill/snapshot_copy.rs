//! Resolve the exact source use of an edge snapshot's bulk copy.
//! The physical read remains CopyBytes; a nonzero static offset reads the
//! binding's pointer in its preceding AddressOffset. Reload that use, never
//! reinterpret the copy's private destination as a semantic source place.
use super::RuntimeSpillError;
use register_model::RegisterOperandAccess;
use selected_instructions::{SelectedBlock, SelectedInstructionKind, VirtualRegisterId};
use semantic_vocabulary::IntegerValue;

fn definition(
    block: &SelectedBlock,
    before: usize,
    register: VirtualRegisterId,
    steps: &mut usize,
    ceiling: u64,
) -> Result<Option<usize>, RuntimeSpillError> {
    for index in (0..before).rev() {
        *steps = steps
            .checked_add(1)
            .ok_or(RuntimeSpillError::IdentityOverflow)?;
        if u64::try_from(*steps).map_err(|_| RuntimeSpillError::IdentityOverflow)? > ceiling {
            return Err(RuntimeSpillError::WorkBudgetExceeded);
        }
        if block.instructions[index].operands.iter().any(|operand| {
            operand.virtual_register == register
                && matches!(
                    operand.access,
                    RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                )
        }) {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

/// Return the instruction that actually reads `register`, or no match for a
/// different binding. A matched copy must prove its exact count and offset;
/// unknown/redefined count or address producers refuse rather than infer bytes
/// from a memory-footprint claim alone. Definition walks are budgeted.
pub(super) fn source_use(
    block: &SelectedBlock,
    copy: usize,
    register: VirtualRegisterId,
    source_offset: u32,
    byte_size: u32,
    steps: &mut usize,
    ceiling: u64,
) -> Result<Option<usize>, RuntimeSpillError> {
    let instruction = &block.instructions[copy];
    let [source, _, count, _, _] = instruction.operands.as_slice() else {
        return Err(RuntimeSpillError::UnsupportedUse);
    };
    if source.access != RegisterOperandAccess::Use || count.access != RegisterOperandAccess::Use {
        return Err(RuntimeSpillError::UnsupportedUse);
    }
    let use_index = if source.virtual_register == register {
        if source_offset != 0 {
            return Err(RuntimeSpillError::UnsupportedUse);
        }
        copy
    } else {
        let Some(index) = definition(block, copy, source.virtual_register, steps, ceiling)? else {
            return Ok(None);
        };
        let address = &block.instructions[index];
        let [input, output] = address.operands.as_slice() else {
            return Ok(None);
        };
        if input.virtual_register != register {
            return Ok(None);
        }
        if address.kind
            != (SelectedInstructionKind::AddressOffset {
                byte_offset: source_offset,
            })
            || source_offset == 0
            || input.access != RegisterOperandAccess::Use
            || output.virtual_register != source.virtual_register
            || output.access != RegisterOperandAccess::Def
        {
            return Err(RuntimeSpillError::UnsupportedUse);
        }
        index
    };
    let Some(index) = definition(block, copy, count.virtual_register, steps, ceiling)? else {
        return Err(RuntimeSpillError::UnsupportedUse);
    };
    let materialize = &block.instructions[index];
    if materialize.kind
        != (SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(byte_size)),
        })
        || !matches!(materialize.operands.as_slice(), [output]
        if output.virtual_register == count.virtual_register && output.access == RegisterOperandAccess::Def)
    {
        return Err(RuntimeSpillError::UnsupportedUse);
    }
    Ok(Some(use_index))
}
