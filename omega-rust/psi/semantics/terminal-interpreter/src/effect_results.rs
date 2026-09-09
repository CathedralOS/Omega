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
            && (contains_bounded_integer(&self.structural_types, result.structural_type)
                || self.structural_values.contains_key(&result.place)
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

/// Opaque host values carry neither complete scalar contents nor selected sum
/// payloads. Type identity alone cannot establish a numeric field restriction.
pub(super) fn contains_bounded_integer(
    types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Some(declaration) = types.get(&current) else {
            return true;
        };
        let mut fields = Vec::new();
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_) => {}
            StructuralTypeShape::FixedArray { element, .. } => pending.push(*element),
            StructuralTypeShape::Record { fields: members } => fields.extend(members),
            StructuralTypeShape::Sum { cases } => {
                fields.extend(cases.iter().flat_map(|case| &case.fields));
            }
            StructuralTypeShape::Mixed {
                fields: members,
                cases,
            } => {
                fields.extend(members);
                fields.extend(cases.iter().flat_map(|case| &case.fields));
            }
        }
        for field in fields {
            match field.field_type {
                terminal_psi::StructuralFieldType::BoundedInteger(_) => return true,
                terminal_psi::StructuralFieldType::Structural(child) => pending.push(child),
                terminal_psi::StructuralFieldType::Scalar(_)
                | terminal_psi::StructuralFieldType::IeeeFloat(_)
                | terminal_psi::StructuralFieldType::ByteSequence(_)
                | terminal_psi::StructuralFieldType::Erased { .. } => {}
            }
        }
    }
    false
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
