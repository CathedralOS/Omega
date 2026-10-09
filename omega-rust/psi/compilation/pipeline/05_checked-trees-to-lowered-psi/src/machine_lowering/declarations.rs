//! Close source declaration identities while the lowering still owns its map.

use checked_trees::CheckedTrees;
use symbols::SymbolHandle;
use terminal_psi::{
    CheckedMachineDeclaration, CheckedMachineSpecialization, NominalDeclarationIdentity,
};

pub(super) fn identity(
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    machine: SymbolHandle,
) -> Result<CheckedMachineDeclaration, &'static str> {
    let nominal = |symbol| {
        checked
            .exact_owner_nominal_identity(symbol, sources)
            .ok_or("checked machine has no exact declaration owner")
            .and_then(|identity| NominalDeclarationIdentity::new(identity.into_string()))
    };
    let declaration = checked
        .machine(machine)
        .ok_or("checked machine declaration is absent")?;
    let entry = checked
        .machine_states(declaration)
        .first()
        .ok_or("checked machine declaration has no entry state")?;
    let contract = checked
        .facts
        .contract_plans
        .for_machine(machine)
        .ok_or("checked machine declaration has no contract")?
        .commitment
        .as_bytes();
    let specialization = checked
        .typed
        .machine_specializations
        .iter()
        .find(|row| row.instance == machine)
        .map(|row| {
            let identity = checked
                .facts
                .specialization_identities
                .for_instance(machine)
                .ok_or("checked machine declaration has no specialization identity")?;
            Ok::<_, &'static str>(CheckedMachineSpecialization {
                template: nominal(row.template)?,
                commitment: identity.commitment.as_bytes(),
            })
        })
        .transpose()?;
    Ok(CheckedMachineDeclaration {
        machine: nominal(machine)?,
        state: nominal(entry.symbol)?,
        contract,
        specialization,
    })
}

pub(super) fn retain(
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    owners: &[(SymbolHandle, semantic_vocabulary::MachineId)],
    module: &mut terminal_psi::TerminalModule,
) -> Result<(), &'static str> {
    // Producer ownership rows follow emitted-machine order. Advance through
    // the output once instead of rebuilding an index or restarting each search.
    let mut machines = module.machines.iter_mut();
    for (source, machine) in owners {
        let declaration = identity(checked, sources, *source)?;
        let emitted = machines
            .find(|row| row.id == *machine)
            .ok_or("checked declaration lost its emitted machine")?;
        emitted.declaration = Some(declaration);
    }
    Ok(())
}
