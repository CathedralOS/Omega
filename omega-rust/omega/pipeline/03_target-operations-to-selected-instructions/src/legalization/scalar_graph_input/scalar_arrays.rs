//! Independently reconstruct array storage from the retained semantic type.
//! Empty extents still traverse every dimension; layout cannot authorize a leaf
//! substitution or replace an array with an equally sized record or sum.
use super::{AbstractOperationPlan, ScalarType, ValueShape, scalar_shape};
use crate::LegalizationError;

pub(in crate::legalization) fn shape(
    result: &terminal_psi::StructuralOperationResult,
    plan: &AbstractOperationPlan,
) -> Result<(ScalarType, u64, ValueShape), LegalizationError> {
    if result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
    {
        return Err(LegalizationError::custody());
    }
    let mut current = result.structural_type;
    let mut count = Some(1_u64);
    let mut empty = false;
    let mut array = false;
    for _ in 0..plan.structural_types.len() {
        let mut matching = plan
            .structural_types
            .iter()
            .filter(|declaration| declaration.id == current);
        let declaration = matching.next().ok_or(LegalizationError::custody())?;
        if matching.next().is_some() {
            return Err(LegalizationError::custody());
        }
        match declaration.shape {
            terminal_psi::StructuralTypeShape::FixedArray { element, length } => {
                array = true;
                empty |= length == 0;
                count = count.and_then(|count| count.checked_mul(length));
                current = element;
            }
            terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type) if array => {
                let leaf = scalar_shape(scalar_type).ok_or(LegalizationError::custody())?;
                let count = if empty {
                    0
                } else {
                    count.ok_or(LegalizationError::custody())?
                };
                let size = count
                    .checked_mul(u64::from(leaf.byte_size))
                    .and_then(|size| u16::try_from(size).ok())
                    .ok_or(LegalizationError::custody())?;
                return Ok((
                    scalar_type,
                    count,
                    // Zero-byte ABI values have canonical alignment one; the
                    // retained structural type still owns dimensions and carrier.
                    ValueShape::integer(size, if size == 0 { 1 } else { leaf.alignment }),
                ));
            }
            _ => return Err(LegalizationError::custody()),
        }
    }
    Err(LegalizationError::custody())
}
