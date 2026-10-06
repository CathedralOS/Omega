use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::ExecutableMachine;
use crate::execution::TerminalExecution;
use crate::values::TerminalScalarValue;
use semantic_vocabulary::StructuralTypeId;
use semantic_vocabulary::ValueId;
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::StructuralParameterDeclaration;
/// Exact row-major primitive payload; the type retains all array dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarArrayValue {
    pub structural_type: StructuralTypeId,
    pub elements: Vec<TerminalScalarValue>,
}

/// An owned unrestricted array returned without claims or qualifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalScalarArrayResult {
    pub value: TerminalScalarArrayValue,
}

impl TerminalExecution {
    /// Copy a complete unrestricted actual into its exact callee parameter.
    /// Opaque host roots and projected/borrowed storage carry no array payload.
    pub(crate) fn prepare_scalar_array_argument(
        &self,
        machine: &ExecutableMachine,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
    ) -> Result<TerminalScalarArrayValue, TerminalInterpretError> {
        let value = self
            .scalar_array_values
            .get(&argument.place)
            .ok_or_else(malformed)?;
        let (scalar_type, count) = terminal_semantics::scalar_array_leaf_shape(
            self.program.structural_types.values(),
            parameter.structural_type,
        )
        .ok_or_else(malformed)?;
        // A shared loan binds the same element values read-only: lending the
        // caller's array is observationally a copy, so `&arr` admits alongside
        // the owned hand-off. Mutable views stay owned-only — writes through a
        // borrow would never reach the caller's storage.
        let owned_handoff = parameter.access == StructuralAccess::Owned
            && argument.access == StructuralAccess::Owned;
        let shared_loan = parameter.access == StructuralAccess::SharedBorrow
            && argument.access == StructuralAccess::SharedBorrow;
        if (!owned_handoff && !shared_loan)
            || !argument.path.is_empty()
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || value.structural_type != parameter.structural_type
            || u64::try_from(value.elements.len()).ok() != Some(count)
            || value
                .elements
                .iter()
                .any(|value| value.scalar_type() != scalar_type)
            || machine
                .entry_claims
                .iter()
                .any(|claim| claim.input == parameter.place)
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == parameter.place)
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(argument.place))
            || self.structural_values.contains_key(&argument.place)
            || self.scalar_case_values.contains_key(&argument.place)
        {
            return Err(malformed());
        }
        Ok(value.clone())
    }

    pub(crate) fn execute_scalar_array_establishment(
        &mut self,
        operation: &terminal_psi::Operation,
        elements: &[ValueId],
    ) -> Result<(), TerminalInterpretError> {
        let result = operation.result.structural().ok_or_else(malformed)?;
        let (scalar_type, count) = terminal_semantics::scalar_array_leaf_shape(
            self.program.structural_types.values(),
            result.structural_type,
        )
        .ok_or_else(malformed)?;
        // An unrestricted payload can be re-established by a cyclic
        // traversal: the fresh establishment replaces the stored elements,
        // matching the per-iteration binding the verifier's cyclic
        // eligibility admits. Unrestricted copies never alias the stored
        // slot, so overwriting retires nothing the caller could still see —
        // unless a live claim still names the place.
        if result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || u64::try_from(elements.len()).ok() != Some(count)
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(result.place))
            || self.structural_values.contains_key(&result.place)
            || self.scalar_case_values.contains_key(&result.place)
        {
            return Err(malformed());
        }
        let mut contents = Vec::with_capacity(elements.len());
        for element in elements {
            let value = self.scalar_value(*element)?;
            if value.scalar_type() != scalar_type {
                return Err(malformed());
            }
            contents.push(value);
        }
        self.scalar_array_values.insert(
            result.place,
            TerminalScalarArrayValue {
                structural_type: result.structural_type,
                elements: contents,
            },
        );
        Ok(())
    }
}
