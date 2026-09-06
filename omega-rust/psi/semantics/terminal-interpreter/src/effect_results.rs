//! Host result carriers and their exact binding to a verified boundary result.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEffectResult {
    Unit,
    Scalar(TerminalScalarValue),
    /// The same opaque, target-neutral value used for structural entry inputs.
    /// This does not provide a sum discriminator or materialized payload fields.
    Structural(TerminalStructuralValue),
}

impl TerminalExecution {
    pub(super) fn preflight_boundary_result(
        &self,
        result: &OperationResult,
    ) -> Result<(), TerminalInterpretError> {
        if let OperationResult::Structural(result) = result
            && (self.structural_values.contains_key(&result.place)
                || self.payloadless_case_values.contains_key(&result.place)
                || self
                    .live_affine_frontier
                    .iter()
                    .any(|value| value.place == result.place)
                || result.multiplicity == StructuralMultiplicity::Linear
                || !result.claims.is_empty()
                || !result.projected_qualifications.is_empty())
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        Ok(())
    }
}

pub(super) fn commit_boundary_result(
    values: &mut BTreeMap<ValueId, TerminalScalarValue>,
    structural_values: &mut BTreeMap<PlaceId, TerminalStructuralValue>,
    live_affine_frontier: &mut BTreeSet<StructuralAffineDiscard>,
    result: &OperationResult,
    expected: &BoundaryMachineResult,
    returned: TerminalEffectResult,
) -> Result<(), TerminalInterpretError> {
    match (result, expected, returned) {
        (OperationResult::Unit, BoundaryMachineResult::Unit, TerminalEffectResult::Unit) => {}
        (
            OperationResult::Scalar(declaration),
            BoundaryMachineResult::Scalar(expected),
            TerminalEffectResult::Scalar(value),
        ) if declaration.scalar_type == *expected && value.scalar_type() == *expected => {
            values.insert(declaration.id, value);
        }
        (
            OperationResult::Structural(declaration),
            BoundaryMachineResult::Structural(expected),
            TerminalEffectResult::Structural(value),
        ) if declaration.structural_type == expected.structural_type
            && declaration.multiplicity == expected.multiplicity
            && declaration.qualifications == expected.qualifications
            && value.structural_type == expected.structural_type
            && value.qualifications == expected.qualifications
            && value.path.is_empty() =>
        {
            structural_values.insert(declaration.place, value);
            if declaration.multiplicity == StructuralMultiplicity::Affine {
                live_affine_frontier.insert(StructuralAffineDiscard {
                    place: declaration.place,
                    path: Vec::new(),
                    structural_type: declaration.structural_type,
                });
            }
        }
        _ => return Err(TerminalInterpretError::VerifiedOperationMalformed),
    }
    Ok(())
}
