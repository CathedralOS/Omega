//! Check allocator register copies without assigning meaning to scratch values.
use super::{Error, Expressions, RegisterOperandAccess, SelectedInstruction};

impl Expressions<'_> {
    pub(super) fn register_copy(&self, row: &SelectedInstruction) -> Result<bool, Error> {
        let [source, destination] = row.operands.as_slice() else {
            return Ok(false);
        };
        if source.operand != 0
            || source.access != RegisterOperandAccess::Use
            || destination.operand != 1
            || destination.access != RegisterOperandAccess::Def
            || !row.implicit_uses.is_empty()
            || !row.implicit_defs.is_empty()
            || !row.clobbers.is_empty()
        {
            return Ok(false);
        }
        let (Some(from), Some(to)) = (
            self.function.virtual_register(source.virtual_register),
            self.function.virtual_register(destination.virtual_register),
        ) else {
            return Ok(false);
        };
        if from.scalar_type != to.scalar_type
            || from.class != to.class
            || source.class != from.class
            || destination.class != to.class
            || self.defining_instruction(to.id)? != row.id
        {
            return Ok(false);
        }
        // This proves only that allocation copies an available value into its
        // uniquely defined destination, with no memory or implicit CPU effect.
        // Semantic consumers still reconstruct the copied value from its input.
        self.require_available(from.id, row.id)?;
        Ok(true)
    }
}
