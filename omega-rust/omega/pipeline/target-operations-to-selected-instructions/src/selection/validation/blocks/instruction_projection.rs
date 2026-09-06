use crate::selection::constraints::row;
use crate::selection::shared::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::selection::validation) fn validate(
    function: usize,
    instruction: &SelectedInstruction,
    id: SelectedInstructionId,
    kind: SelectedInstructionKind,
    key: RegisterConstraintKey,
    registers: &[VirtualRegisterId],
    provenance: &SelectedInstructionProvenance,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let constraint = row(catalog, key)?;
    if instruction.id != id
        || instruction.kind != kind
        || instruction.constraint != key
        || instruction.provenance != *provenance
        || instruction.operands.len() != registers.len()
        || instruction
            .operands
            .iter()
            .zip(registers)
            .zip(&constraint.operands)
            .any(|((operand, register), expected)| {
                operand.virtual_register != *register
                    || operand.operand != expected.operand
                    || operand.access != expected.access
                    || operand.class != expected.class
                    || operand.fixed_view != expected.fixed_view
                    || operand.tied_to != expected.tied_to
                    || operand.early_clobber != expected.early_clobber
            })
    {
        return Err(SelectedInstructionError::InstructionProjectionMismatch {
            function,
            instruction: id.0,
        });
    }
    if instruction.implicit_uses != constraint.implicit_uses
        || instruction.implicit_defs != constraint.implicit_defs
        || instruction.clobbers != constraint.clobbers
    {
        return Err(SelectedInstructionError::ConstraintEffectMismatch {
            function,
            instruction: id.0,
        });
    }
    Ok(())
}
