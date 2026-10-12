//! Complete byte operands share construction checks across record and case fields.
//! Return immutable backing only after the actual carrier, predicates, extent
//! and custody agree; the caller stages it under its exact field namespace.
use super::storage::ConstructorValue;
use crate::byte_sequences::ByteSequenceView;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use std::collections::BTreeSet;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralFieldDeclaration, StructuralTypeShape,
};

impl TerminalExecution<'_> {
    pub(crate) fn prepare_constructor_byte_field(
        &self,
        declaration: &StructuralFieldDeclaration,
        argument: &StructuralArgument,
        child: &ConstructorValue<'_>,
        frontier: &mut BTreeSet<StructuralAffineDiscard>,
        consumed: &mut Vec<StructuralArgument>,
    ) -> Result<Option<ByteSequenceView>, TerminalInterpretError> {
        let Some(carrier) = declaration.field_type.byte_sequence_carrier() else {
            return Ok(None);
        };
        let ConstructorValue::Placed(child) = child else {
            return Err(malformed());
        };
        let bytes = self
            .byte_sequence_values
            .get(&argument.place)
            .ok_or_else(malformed)?
            .immutable()?;
        if argument.access != StructuralAccess::Owned || !argument.path.is_empty() {
            return Err(malformed());
        }
        if carrier.is_shared_readable_view() {
            if !self
                .program
                .structural_types()
                .get(&child.structural_type)
                .is_some_and(|row| row.shape.is_shared_readable_byte_view())
                || !terminal_semantics::byte_literal_qualifications_hold_in(
                    self.program.catalogs.structural_domains.iter(),
                    child.structural_type,
                    &child.qualifications,
                    bytes.bytes(),
                )
                || !declaration
                    .field_type
                    .byte_predicates()
                    .holds_for(bytes.bytes())
            {
                return Err(malformed());
            }
            self.stage_constructor_operand(argument, frontier, consumed)?;
        } else if let ByteSequenceCarrier::BoundedOwned { capacity } = carrier {
            let predicates = terminal_semantics::byte_qualification_predicates_in(
                self.program.catalogs.structural_domains.iter(),
                child.structural_type,
                &child.qualifications,
            )
            .ok_or_else(malformed)?;
            if !child.path.is_empty()
                || !self
                    .program
                    .structural_types()
                    .get(&child.structural_type)
                    .is_some_and(|row| row.shape == StructuralTypeShape::ByteSequence(carrier))
                || !predicates.implies(declaration.field_type.byte_predicates())
                || u64::try_from(bytes.len()).map_or(true, |length| length > capacity)
                || self.live_affine_discard_at(argument.place).is_some()
            {
                return Err(malformed());
            }
        } else {
            return Err(malformed());
        }
        Ok(Some(bytes.clone()))
    }
}
