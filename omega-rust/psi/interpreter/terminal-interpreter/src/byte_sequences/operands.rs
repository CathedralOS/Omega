//! Shared operand resolution for byte-sequence operations: one
//! `ByteSequenceOperand` names either a whole view or one structural field's
//! view, and every consumer reads the same live extent bound.

use crate::byte_sequences::ByteSequenceView;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::TerminalScalarValue;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_psi::control_flow::ByteSequenceOperand;

impl TerminalExecution<'_> {
    /// Fixed-array observations use the same primitive backing as indexed
    /// reads. They expose exactly the declared extent and no stored length.
    pub(crate) fn fixed_byte_array_observation(
        &self,
        source: &terminal_psi::StructuralArgument,
    ) -> Result<Option<ByteSequenceView>, TerminalInterpretError> {
        let root = self.live_structural_type(source.place)?;
        let types = self.program.structural_types();
        let selected =
            terminal_semantics::runtime_structural_path_tip(types.values(), root, &source.path)
                .ok_or_else(malformed)?;
        if !matches!(
            selected.shape,
            terminal_psi::StructuralTypeShape::FixedArray { .. }
        ) || terminal_semantics::fixed_byte_extent(types.values(), selected.id).is_none()
        {
            return Ok(None);
        }
        self.validate_descriptor_borrow(source, terminal_psi::StructuralAccess::SharedBorrow)?;
        let snapshot = self.scalar_array_snapshot(source, selected.id)?;
        let bytes = snapshot
            .elements
            .iter()
            .map(crate::structural_inputs::byte_arrays::storage::scalar_byte)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(ByteSequenceView::new(bytes)))
    }

    pub(crate) fn byte_sequence_operand_view(
        &self,
        operand: &ByteSequenceOperand,
    ) -> Result<ByteSequenceView, TerminalInterpretError> {
        let bytes = match operand {
            ByteSequenceOperand::Whole { source, length } => {
                if source.access != terminal_psi::StructuralAccess::SharedBorrow {
                    return Err(malformed());
                }
                if let Some(bytes) = self.fixed_byte_array_observation(source)? {
                    let expected =
                        IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
                    if !matches!(self.values.get(length), Some(TerminalScalarValue::Integer { scalar_type, value: IntegerValue::Unsigned(count) })
                        if *scalar_type == expected && *count == bytes.len() as u128)
                    {
                        return Err(malformed());
                    }
                    return Ok(bytes);
                }
                self.validate_descriptor_borrow(
                    source,
                    terminal_psi::StructuralAccess::SharedBorrow,
                )?;
                let selected = self.resolve_live_projection(
                    source.place,
                    &source.path,
                    terminal_psi::StructuralAccess::SharedBorrow,
                )?;
                let binding = self.byte_descriptor_binding(source, &selected)?;
                // The resolved binding and extent belong to this descriptor,
                // not to the pointer carrier at the operand's root place.
                self.byte_descriptor_length(&binding, &selected)?;
                self.reborrow_byte_sequence_binding(
                    &binding,
                    &selected,
                    terminal_psi::StructuralAccess::SharedBorrow,
                )?
                .immutable()?
                .clone()
            }
            ByteSequenceOperand::Field { source, field, .. } => {
                let (key, capacity) =
                    self.readable_byte_field(source.place, &source.path, *field)?;
                let bytes = self.byte_field_snapshot(&key)?;
                if capacity.is_some_and(|capacity| bytes.len() as u128 > u128::from(capacity)) {
                    return Err(malformed());
                }
                bytes.clone()
            }
        };
        let expected = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
        if !matches!(self.values.get(&operand.length()), Some(TerminalScalarValue::Integer {
            scalar_type, value: IntegerValue::Unsigned(length),
        }) if *scalar_type == expected && *length == bytes.len() as u128)
        {
            return Err(malformed());
        }
        Ok(bytes)
    }
}
