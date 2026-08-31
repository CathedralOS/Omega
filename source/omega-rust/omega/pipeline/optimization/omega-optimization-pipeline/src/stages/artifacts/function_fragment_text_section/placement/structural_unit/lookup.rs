use psi_core::MachineId;

use super::super::super::RelocationFreeTextSectionPlacementError;

pub(super) fn unique_machine<T>(
    functions: &[T],
    machine: MachineId,
    identify: impl Fn(&T) -> MachineId,
) -> Result<&T, RelocationFreeTextSectionPlacementError> {
    let mut matches = functions
        .iter()
        .filter(|function| identify(function) == machine);
    let function = matches
        .next()
        .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?;
    if matches.next().is_some() {
        return Err(RelocationFreeTextSectionPlacementError::DuplicateFunction(
            machine,
        ));
    }
    Ok(function)
}
