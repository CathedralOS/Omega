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
                capture: capture_identity(identities, checked, machine)?,
                template: nominal(identities, checked, sources, template)?,
                template_callable: identities.intern(
                    &checked
                        .machine(template)
                        .and_then(|machine| checked.typed.machine_callable_identity_text(machine))
                        .ok_or("checked specialization lost its exact template callable")?,
                ),
                commitment: identity.commitment.as_bytes(),
            })
        })
        .transpose()?;
    Ok(CheckedMachineDeclaration {
        machine: nominal(identities, checked, sources, machine)?,
        state: nominal(identities, checked, sources, entry.symbol)?,
        semantic_declaration,
        parameter_types,
        callable,
        result_type,
        contract,
        specialization,
    })
}

pub(crate) fn capture_identity(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: SymbolHandle,
) -> Result<Option<terminal_psi::CheckedMachineCapture>, &'static str> {
    let template = checked.executable_application_template(machine)?;
    let mut direct = checked
        .captured_machine_applications
        .iter()
        .filter(|(_, capture)| capture.instance == machine)
        .map(|(_, capture)| capture);
    let direct_capture = direct.next();
    if direct.next().is_some() {
        return Err("checked declaration has ambiguous capture ownership");
    }
    let capture = if direct_capture.is_some() {
        direct_capture
    } else {
        let mut outer = checked
            .captured_machine_applications
            .iter()
            .filter(|(_, capture)| Some(capture.instance) == template)
            .map(|(_, capture)| capture);
        let capture = outer.next();
        if outer.next().is_some() {
            return Err("checked invocation has ambiguous capture ownership");
        }
        capture
    };
    capture
        .map(|capture| {
            let receipt = checked
                .facts
                .specialization_identities
                .for_instance(capture.instance)
                .ok_or("checked capture declaration lost its independent receipt")?;
            Ok(terminal_psi::CheckedMachineCapture {
                template_identity: identities.intern(&capture.normalized_template_identity),
                template_commitment: receipt.template_contract_commitment.as_bytes(),
                specialization_commitment: receipt.commitment.as_bytes(),
            })
        })
        .transpose()
}

/// Retain a requirement's own checked callable, independently of its selected
/// implementation. Trait signatures have no Machine or entry body: their exact
/// owner/signature coordinates and complete contract already live in the
/// checked requirement capsule. Lower those coordinates rather than borrowing
/// an implementation's declaration or re-deriving a contract from source.
pub(crate) fn boundary_identity(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    target: SymbolHandle,
) -> Result<Option<CheckedMachineDeclaration>, &'static str> {
    if checked.machine(target).is_some() {
        return identity(identities, checked, sources, target).map(Some);
    }
    let program = &checked.typed;
    let mut requirements = program.traits().iter().flat_map(|owner| {
        program
            .trait_machine_instances(owner)
            .filter_map(move |signature| (signature.symbol == target).then_some((owner, signature)))
    });
    let Some((owner, signature)) = requirements.next() else {
        // Synthetic evidence and legacy operator declarations do not masquerade
        // as authored machine or trait requirements.
        return Ok(None);
    };
    if requirements.next().is_some() {
        return Err("boundary requirement has multiple checked declaration owners");
    }
    let capsule = checked
        .facts
        .contract_plans
        .crash_capsule(owner.symbol, signature.symbol)
        .ok_or("boundary requirement lost its complete checked contract capsule")?;
    let contract = capsule.target_contract_commitment();
    if contract.is_zero() {
        return Err("boundary requirement has an empty checked contract commitment");
    }
    let parameter_types = program
        .state_signature_parameters(signature)
        .iter()
        .map(|parameter| {
            identities
                .intern(&program.package_qualified_type_identity_text(parameter.type_reference))
        })
        .collect();
    Ok(Some(CheckedMachineDeclaration {
        machine: nominal(identities, checked, sources, owner.symbol)?,
        state: nominal(identities, checked, sources, signature.symbol)?,
        semantic_declaration: identities.intern(
            &program
                .normalized_hermetic_symbol_identity(signature.symbol)
                .map_err(|_| "boundary requirement has no hermetic declaration identity")?,
        ),
        parameter_types,
        callable: identities
            .intern(&program.trait_requirement_callable_identity_text(owner, signature)),
        result_type: identities
            .intern(&program.package_qualified_type_identity_text(signature.return_type)),
        contract: contract.as_bytes(),
        specialization: None,
    }))
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
