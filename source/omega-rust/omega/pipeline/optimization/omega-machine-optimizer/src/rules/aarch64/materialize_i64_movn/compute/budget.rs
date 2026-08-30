use crate::{Aarch64MovnMaterializationError, Aarch64MovnMaterializationWorkAxis};

pub(super) fn charge(
    usage: &mut u64,
    budget: u64,
    axis: Aarch64MovnMaterializationWorkAxis,
) -> Result<(), Aarch64MovnMaterializationError> {
    *usage = usage
        .checked_add(1)
        .ok_or(Aarch64MovnMaterializationError::BudgetExceeded(axis))?;
    if *usage > budget {
        return Err(Aarch64MovnMaterializationError::BudgetExceeded(axis));
    }
    Ok(())
}
