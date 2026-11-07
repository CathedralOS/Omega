//! Bounded exact-extent copy preparation shared by ordinary homes and edge snapshots.
//! Callers retain endpoint formation, memory roles and ordering; this owner only
//! chooses the inline split and prepares the existing admitted bulk-copy operands.
use crate::SelectedInstructionError;
use register_model::{
    RegisterConstraintKey, RegisterOperandAccess, ValidatedRegisterConstraintCatalog,
};
use selected_instructions::{
    SelectedInstructionId, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};

/// At most one page is expanded into straight-line loads and stores.
pub(super) const INLINE_MAX_BYTES: u32 = 4096;

/// Allocation-free word/tail decomposition, used only for bounded inline copies.
pub(super) fn chunks(byte_size: u32) -> impl Iterator<Item = (u32, u8)> {
    let mut offset = 0;
    std::iter::from_fn(move || {
        let width = [8u32, 4, 2, 1]
            .into_iter()
            .find(|width| *width <= byte_size - offset)?;
        let chunk = (offset, width as u8);
        offset += width;
        Some(chunk)
    })
}

/// The three immutable inputs and two private early-clobber words of CopyBytes.
/// Scratch classes come from the same validated row that emits the instruction.
pub(super) fn bulk_operands(
    instruction: SelectedInstructionId,
    inputs: [VirtualRegisterId; 3],
    key: RegisterConstraintKey,
    catalog: &ValidatedRegisterConstraintCatalog,
    registers: &mut Vec<VirtualRegister>,
) -> Result<[VirtualRegisterId; 5], SelectedInstructionError> {
    let row = super::constraints::row(catalog, key)?;
    let reject = || SelectedInstructionError::MissingConstraint(key);
    if row.operands.len() != 5
        || row.operands.iter().enumerate().any(|(index, operand)| {
            operand.operand as usize != index
                || operand.access
                    != if index < 3 {
                        RegisterOperandAccess::Use
                    } else {
                        RegisterOperandAccess::Def
                    }
                || operand.tied_to.is_some()
                || (index >= 3 && (!operand.early_clobber || operand.fixed_view.is_some()))
        })
    {
        return Err(reject());
    }
    let mut operands = [inputs[0], inputs[1], inputs[2], inputs[0], inputs[0]];
    for index in 3..5 {
        let id = VirtualRegisterId(registers.len().try_into().map_err(|_| reject())?);
        registers.push(VirtualRegister {
            id,
            scalar_type: ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| reject())?,
            ),
            class: row.operands[index].class,
            origin: VirtualRegisterOrigin::InstructionScratch {
                instruction,
                operand: index as u16,
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        operands[index] = id;
    }
    Ok(operands)
}
