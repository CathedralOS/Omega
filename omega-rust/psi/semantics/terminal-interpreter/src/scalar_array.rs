use super::*;

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
    pub(super) fn execute_scalar_array_establishment(
        &mut self,
        operation: &terminal_psi::Operation,
        elements: &[ValueId],
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let result = operation.result.structural().ok_or_else(invalid)?;
        let (scalar_type, count) = terminal_semantics::scalar_array_leaf_shape(
            self.structural_types.values(),
            result.structural_type,
        )
        .ok_or_else(invalid)?;
        if result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || u64::try_from(elements.len()).ok() != Some(count)
            || self.scalar_array_values.contains_key(&result.place)
            || self.structural_values.contains_key(&result.place)
            || self.scalar_case_values.contains_key(&result.place)
        {
            return Err(invalid());
        }
        let mut contents = Vec::with_capacity(elements.len());
        for element in elements {
            let value = self
                .values
                .get(element)
                .copied()
                .ok_or(TerminalInterpretError::VerifiedValueMissing(*element))?;
            if value.scalar_type() != scalar_type {
                return Err(invalid());
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
