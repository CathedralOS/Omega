//! Declaration authority shared by nominal cleanup execution and static replay.
//! The target is selected by the subject's unique owner designation. A call site,
//! matching bytes or home retirement cannot manufacture that authority. Dynamic
//! execution separately authenticates the live place, complete claim roster and
//! descendant partition before committing disposal.

use terminal_psi::{
    NominalAffineCleanup, StructuralTypeDeclaration, TerminalMachine, TerminalMachineResult,
};

pub fn target<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration>,
    machines: impl Iterator<Item = &'a TerminalMachine> + Clone,
    cleanup: &NominalAffineCleanup,
) -> Result<&'a TerminalMachine, &'static str> {
    if !types
        .into_iter()
        .any(|declaration| declaration.id == cleanup.structural_type)
    {
        return Err("nominal cleanup subject declaration is absent");
    }
    let target = machines
        .clone()
        .find(|machine| machine.id == cleanup.cleanup_machine)
        .ok_or("nominal cleanup target is absent")?;
    if target.attachment != Some(cleanup.structural_type)
        || target.nominal_cleanup_subject != Some(cleanup.structural_type)
        || machines
            .filter(|machine| machine.nominal_cleanup_subject == Some(cleanup.structural_type))
            .count()
            != 1
    {
        return Err("nominal cleanup lacks its exact unique owner designation");
    }
    if target.result != TerminalMachineResult::Unit
        || !target.parameters.is_empty()
        || !target.entry_claims.is_empty()
        || !target.content_entry_claims.is_empty()
        || target.structural_parameters.len() > 1
        || target
            .structural_parameters
            .iter()
            .any(|parameter| !parameter.is_cleanup_receiver(cleanup))
    {
        return Err("nominal cleanup target has an incompatible receiver or result");
    }
    if cleanup
        .consumed_claims
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err("nominal cleanup claim roster is not strictly canonical");
    }
    Ok(target)
}
