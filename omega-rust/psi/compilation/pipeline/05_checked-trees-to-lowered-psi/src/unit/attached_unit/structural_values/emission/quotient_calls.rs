//! Unit construction supplies completed operands to the shared call emitter.
use super::*;
impl Emission<'_, '_, '_> {
    pub(super) fn quotient_call(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        value: CheckedStructuralValueHandle,
        arguments: &[CheckedStructuralValueHandle],
    ) -> Result<PlaceId, LoweringError> {
        let mut actuals = Vec::new();
        for argument in arguments {
            let child = self.structural_value_node(*argument);
            let ty = lookup_type_id(
                self.type_ids,
                &validation::stored_structural_type_identity(
                    &self.checked.typed,
                    child.type_reference,
                    &[],
                    &[],
                ),
            )?;
            let child = self.child_constructor_operand(
                identities,
                ty,
                *argument,
                child.type_reference,
                &[],
            )?;
            if !child.claims.is_empty() || !child.argument.path.is_empty() {
                return unsupported("quotient invocation input carries custody");
            }
            actuals.push((child.argument, ty));
        }
        let place = place_id(allocate_dense(self.next_place)?);
        let declaration = crate::emission::quotient_values::emit_call(
            self.checked,
            value,
            &actuals,
            place,
            self.structural_type,
            self.next_value,
            self.operations,
            self.calls,
        )?;
        self.temporary_places.push(declaration);
        Ok(place)
    }
}
