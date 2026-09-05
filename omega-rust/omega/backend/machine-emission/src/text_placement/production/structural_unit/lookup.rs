use semantic_vocabulary::MachineId;

use super::super::super::TextPlacementError;

pub(super) fn unique_machine<T>(
    functions: &[T],
    machine: MachineId,
    identify: impl Fn(&T) -> MachineId,
) -> Result<&T, TextPlacementError> {
    let mut matches = functions
        .iter()
        .filter(|function| identify(function) == machine);
    let function = matches
        .next()
        .ok_or(TextPlacementError::SourceShapeMismatch)?;
    if matches.next().is_some() {
        return Err(TextPlacementError::DuplicateFunction(machine));
    }
    Ok(function)
}
