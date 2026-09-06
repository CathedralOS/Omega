//! Ordered allocation of exact structural operation-result homes.

use crate::assignment::shared::*;

pub(super) fn assign(
    operation: OperationId,
    requirement: &target_operations::TargetStructuralHomeRequirement,
    homes: &mut BTreeMap<PlaceId, AssignedStructuralHome>,
    next_home: &mut u32,
) -> Result<AssignedStructuralHome, AssignmentError> {
    let shape = requirement.layout.shape();
    if requirement.defining_operation != operation
        || shape.byte_size == 0
        || shape.alignment == 0
        || homes.contains_key(&requirement.result.place)
    {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    let alignment = match requirement.layout {
        target_operations::TargetStructuralHomeLayout::Aggregate(_) => shape.alignment.max(8),
        target_operations::TargetStructuralHomeLayout::Sum(_) => shape.alignment,
    };
    *next_home = super::scalar_call::align_unit_frame_offset(*next_home, u32::from(alignment))?;
    let home = AssignedStructuralHome {
        requirement: requirement.clone(),
        byte_offset: *next_home,
    };
    *next_home = next_home
        .checked_add(u32::from(shape.byte_size))
        .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
    homes.insert(requirement.result.place, home.clone());
    Ok(home)
}
