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
    if let Some(identity) = invocation_local(identities, checked, symbol)? {
        return Ok(NominalDeclarationIdentity::new(identity));
    }
    let identity = checked
        .exact_owner_nominal_identity(symbol, sources)
        .ok_or("checked declaration has no exact package or toolchain owner")?;
    let text = checked
        .type_identity_text(identity)
        .ok_or("checked declaration lost its normalized identity")?;
    Ok(NominalDeclarationIdentity::new(identities.intern(&text)))
}

/// Missing package ownership is permitted only for an explicitly private code
/// product. The checked symbol coordinate stays local to that code's live owner;
/// it is never reclassified as a package or toolchain declaration.
fn invocation_local(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    symbol: SymbolHandle,
) -> Result<Option<terminal_psi::TerminalIdentityIndex>, &'static str> {
    if !symbol.is_valid()
        || !identities.is_invocation_scoped()
        || checked.symbols.symbol_package_identity(symbol).is_some()
        || checked.symbols.symbol_source_origin(symbol) != Some(source::SourceOrigin::User)
    {
        return Ok(None);
    }
    let coordinate = format!(
        "invocation-declaration({}:{})",
        symbol.arena_index(),
        symbol.generation()
    );
    identities.intern_invocation_local(&coordinate).map(Some)
}

pub(crate) fn semantic_identity(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    symbol: SymbolHandle,
) -> Result<terminal_psi::TerminalIdentityIndex, String> {
    if let Some(identity) = invocation_local(identities, checked, symbol)? {
        return Ok(identity);
    }
    Ok(identities.intern(&checked.normalized_hermetic_symbol_identity(symbol)?))
}

pub(crate) fn identity(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    machine: SymbolHandle,
) -> Result<CheckedMachineDeclaration, String> {
    let declaration = checked
        .facts
        .declaration_descriptions
        .for_machine(machine)?;
    let parameters = checked
        .facts
        .declaration_descriptions
        .parameters
        .span(declaration.parameters)
        .ok_or("checked declaration lost its retained parameter identities")?;
    // Entry receipts retain the authored parameter type, including its
    // reference access. Structural declarations name only the owned referent,
    // so their identities cannot stand in for this source-signature identity.
    for parameter in parameters {
        type_identity(identities, checked, parameter.normalized)?;
    }
    let semantic_declaration = semantic_identity(identities, checked, machine)
        .map_err(|reason| format!("checked machine identity: {reason}"))?;
    let parameter_types = parameters
        .iter()
        .map(|parameter| type_identity(identities, checked, parameter.qualified))
        .collect::<Result<Vec<_>, _>>()?;
    let callable = identities.intern(&checked.callable_identity_text(declaration.callable));
    let result_type = type_identity(identities, checked, declaration.result)?;
    let specialization = declaration
        .specialization
        .as_ref()
        .map(|specialization| {
            Ok::<_, String>(CheckedMachineSpecialization {
                capture: capture_identity(identities, checked, machine)?,
                template: nominal(identities, checked, sources, specialization.template)?,
                template_semantic_declaration: semantic_identity(
                    identities,
                    checked,
                    specialization.template,
                )?,
                template_callable: identities
                    .intern(&checked.callable_identity_text(specialization.callable)),
                commitment: specialization.commitment,
            })
        })
        .transpose()?;
    Ok(CheckedMachineDeclaration {
        machine: nominal(identities, checked, sources, machine)?,
        state: nominal(identities, checked, sources, declaration.entry)?,
        semantic_declaration,
        parameter_types,
        callable,
        result_type,
        contract: declaration.contract.as_bytes(),
        specialization,
    })
}

fn type_identity(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    identity: typed_trees::type_identity::TypeIdentityId,
) -> Result<terminal_psi::TerminalIdentityIndex, &'static str> {
    let text = checked
        .type_identity_text(identity)
        .ok_or("checked declaration lost its retained type identity")?;
    Ok(identities.intern(&text))
}

pub(crate) fn capture_identity<'a>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &'a CheckedTrees,
    machine: SymbolHandle,
) -> Result<Option<terminal_psi::CheckedMachineCapture>, &'a str> {
    checked
        .facts
        .declaration_descriptions
        .for_machine(machine)?
        .specialization
        .as_ref()
        .and_then(|specialization| specialization.capture.as_ref())
        .map(|capture| {
            Ok(terminal_psi::CheckedMachineCapture {
                template_identity: type_identity(identities, checked, capture.template_identity)?,
                template_commitment: capture.template_contract,
                specialization_commitment: capture.specialization,
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
) -> Result<Option<CheckedMachineDeclaration>, String> {
    if checked
        .facts
        .declaration_descriptions
        .description(target)
        .is_some()
    {
        return identity(identities, checked, sources, target).map(Some);
    }
    let program = &checked.typed;
    let mut requirements = program.traits().iter().flat_map(|owner| {
        program
            .executable_trait_machine_instances(owner)
            .filter_map(move |signature| (signature.symbol == target).then_some((owner, signature)))
    });
    let Some((owner, signature)) = requirements.next() else {
        // Synthetic evidence and legacy operator declarations do not masquerade
        // as authored machine or trait requirements.
        return Ok(None);
    };
    if requirements.next().is_some() {
        return Err("boundary requirement has multiple checked declaration owners".into());
    }
    let capsule = checked
        .facts
        .contract_plans
        .crash_capsule(owner.symbol, signature.symbol)
        .ok_or("boundary requirement lost its complete checked contract capsule")?;
    let contract = capsule.target_contract_commitment();
    if contract.is_zero() {
        return Err("boundary requirement has an empty checked contract commitment".into());
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
        semantic_declaration: semantic_identity(identities, checked, signature.symbol)
            .map_err(|reason| format!("boundary requirement identity: {reason}"))?,
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
) -> Result<(), String> {
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
