//! Complete byte operands share construction checks across record and case fields.
//! Retain owned payloads or borrowed descriptors only after the actual carrier,
//! predicates, extent and custody agree. Exclusive bindings move without
//! observing/copying their backing; the caller stages the exact field slot.
use super::storage::ConstructorValue;
use crate::byte_sequences::binding::ByteSequenceBinding;
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
    ) -> Result<Option<ByteSequenceBinding>, TerminalInterpretError> {
        let Some(carrier) = declaration.field_type.byte_sequence_carrier() else {
            return Ok(None);
        };
        let ConstructorValue::Placed(child) = child else {
            return Err(malformed());
        };
        let binding = self
            .byte_sequence_values
            .get(&argument.place)
            .ok_or_else(malformed)?;
        if argument.access != StructuralAccess::Owned || !argument.path.is_empty() {
            return Err(malformed());
        }
        if carrier.is_shared_readable_view() {
            let bytes = binding.immutable()?;
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
            let bytes = binding.immutable()?;
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
        } else if carrier.is_borrowed() {
            let declaration_type = self.program.structural_types();
            let actual = declaration_type
                .get(&child.structural_type)
                .ok_or_else(malformed)?;
            if terminal_semantics::byte_carrier_field_declaration(
                declaration_type.values(),
                carrier,
            )
            .is_none_or(|expected| expected.id != actual.id)
                || matches!(binding, ByteSequenceBinding::Immutable(_))
                || !child.projected_qualifications.is_empty()
                || !terminal_semantics::byte_qualification_predicates_in(
                    self.program.catalogs.structural_domains.iter(),
                    child.structural_type,
                    &child.qualifications,
                )
                .ok_or_else(malformed)?
                .implies(declaration.field_type.byte_predicates())
            {
                return Err(malformed());
            }
            binding.validate_mutable_referent(self, child)?;
            let length = self.byte_descriptor_length(binding, child)?;
            if matches!(carrier, ByteSequenceCarrier::BoundedView { capacity, .. } if length != capacity)
                || !self.stage_constructor_operand(argument, frontier, consumed)?
            {
                return Err(malformed());
            }
        } else {
            return Err(malformed());
        }
        Ok(Some(binding.clone()))
    }
}
