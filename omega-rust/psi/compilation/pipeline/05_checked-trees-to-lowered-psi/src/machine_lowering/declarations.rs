//! Close source declaration identities while the lowering still owns its map.

use checked_trees::CheckedTrees;
use symbols::SymbolHandle;
use terminal_psi::{
    CheckedMachineDeclaration, CheckedMachineSpecialization, NominalDeclarationIdentity,
};

pub(crate) fn nominal(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    symbol: SymbolHandle,
) -> Result<NominalDeclarationIdentity, &'static str> {
    let identity = checked
        .exact_owner_nominal_identity(symbol, sources)
        .ok_or("checked declaration has no exact package or toolchain owner")?;
    let text = checked
        .type_identity_text(identity)
        .ok_or("checked declaration lost its normalized identity")?;
    Ok(NominalDeclarationIdentity::new(identities.intern(&text)))
}

pub(crate) fn identity(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    machine: SymbolHandle,
) -> Result<CheckedMachineDeclaration, &'static str> {
    let declaration = checked
        .machine(machine)
        .ok_or("checked machine declaration is absent")?;
    let entry = checked
        .machine_states(declaration)
        .first()
        .ok_or("checked machine declaration has no entry state")?;
    // Entry receipts retain the authored parameter type, including its
    // reference access. Structural declarations name only the owned referent,
    // so their identities cannot stand in for this source-signature identity.
    for parameter in checked.state_parameters(entry) {
        identities.intern(&checked.normalized_type_identity_text(parameter.type_reference));
    }
    let semantic_declaration = identities.intern(
        &checked
            .normalized_hermetic_symbol_identity(machine)
            .map_err(|_| "checked machine has no hermetic declaration identity")?,
    );
    let parameter_types = checked
        .state_parameters(entry)
        .iter()
        .map(|parameter| {
            identities
                .intern(&checked.package_qualified_type_identity_text(parameter.type_reference))
        })
        .collect();
    let callable = identities.intern(
        &checked
            .typed
            .machine_callable_identity_text(declaration)
            .ok_or("checked machine has no callable signature identity")?,
    );
    let result_type = identities.intern(
        &checked
            .typed
            .package_qualified_type_identity_text(entry.return_type),
    );
    let mut nominal = |symbol| nominal(identities, checked, sources, symbol);
    let contract = checked
        .facts
        .contract_plans
        .for_machine(machine)
        .ok_or("checked machine declaration has no contract")?
        .commitment
        .as_bytes();
    let specialization = checked
        .typed
        .executable_application_template(machine)?
        .map(|template| {
            let identity = checked
                .facts
                .specialization_identities
                .for_instance(machine)
                .ok_or("checked executable application has no retained identity")?;
            Ok::<_, &'static str>(CheckedMachineSpecialization {
                template: nominal(template)?,
                commitment: identity.commitment.as_bytes(),
            })
        })
        .transpose()?;
    Ok(CheckedMachineDeclaration {
        machine: nominal(machine)?,
        state: nominal(entry.symbol)?,
        semantic_declaration,
        parameter_types,
        callable,
        result_type,
        contract,
        specialization,
    })
}

pub(super) fn retain(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    owners: &[(SymbolHandle, semantic_vocabulary::MachineId)],
    module: &mut terminal_psi::TerminalModule,
) -> Result<(), &'static str> {
    // Producer ownership rows follow emitted-machine order. Advance through
    // the output once instead of rebuilding an index or restarting each search.
    let mut machines = module.machines.iter_mut();
    for (source, machine) in owners {
        let declaration = identity(identities, checked, sources, *source)?;
        let emitted = machines
            .find(|row| row.id == *machine)
            .ok_or("checked declaration lost its emitted machine")?;
        emitted.declaration = Some(declaration);
    }
    Ok(())
}
