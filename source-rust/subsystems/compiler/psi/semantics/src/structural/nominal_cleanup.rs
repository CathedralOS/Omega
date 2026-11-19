use terminal_psi::{StructuralTypeDeclaration, TerminalMachine, TerminalMachineResult};

/// Authenticate declaration selections once before replaying operations.
/// A claimed hook cannot make copyable or no-code storage acquire a new
/// disposition, and an absent identity supplies no declaration evidence.
pub fn validate_selections(module: &terminal_psi::TerminalModule) -> Result<(), &'static str> {
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<std::collections::BTreeMap<_, _>>();
    if machines.len() != module.machines.len() {
        return Err("nominal cleanup has duplicate machine identities");
    }
    for declaration in &module.structural_types {
        let Some(selection) = &declaration.nominal_cleanup else {
            continue;
        };
        if selection.declaration.identity() != declaration.identity
            || declaration.is_copyable()
            || declaration.no_code_affine_cleanup
            || module
                .identities
                .text(selection.declaration.identity())
                .is_none_or(str::is_empty)
        {
            return Err("nominal cleanup lost its exact carrier declaration or disposition");
        }
        crate::closed_conformances::identity_package(
            &module.identities,
            selection.declaration.identity(),
        )?;
        let target = machines
            .get(&selection.cleanup_machine)
            .ok_or("selected owned cleanup target is absent")?;
        validate_owned_target(declaration, target)?;
    }
    Ok(())
}

/// Automatic cleanup must finish normally on every path. Check the
/// actual closed bodies without publishing a stronger contract; calls
/// through a requirement still need its public termination promise.
pub fn validate_normal_returns(
    module: &terminal_psi::TerminalModule,
) -> Result<(), &'static str> {
    use std::collections::BTreeMap;
    use terminal_psi::Terminator;
    let positions: BTreeMap<_, _> = module
        .machines
        .iter()
        .enumerate()
        .map(|(slot, machine)| (machine.id, slot))
        .collect();
    if positions.len() != module.machines.len() {
        return Err("cleanup completion has duplicate machine identities");
    }
    let mut visited = vec![0u8; module.machines.len()];
    for selection in module
        .structural_types
        .iter()
        .filter_map(|ty| ty.nominal_cleanup.as_ref())
    {
        let first = *positions
            .get(&selection.cleanup_machine)
            .ok_or("cleanup completion target is absent")?;
        let mut pending = vec![(first, false)];
        while let Some((slot, returning)) = pending.pop() {
            if returning {
                visited[slot] = 2;
                continue;
            }
            match visited[slot] {
                2 => continue,
                1 => return Err("recursive generated cleanup has no finite completion proof"),
                _ => {}
            }
            let machine = &module.machines[slot];
            let safe_header = |header: &terminal_psi::MachineContract| {
                if header.published_may_block
                    || header.published_may_suspend
                    || !header.crash_routes.is_empty()
                {
                    return Err("automatic cleanup permits waiting or abnormal completion");
                }
                if !header.published_service_ceiling.is_empty()
                    || !header.published_synchronous_entries.is_empty()
                {
                    return Err(
                        "automatic cleanup with service or callback authority is not served",
                    );
                }
                Ok(())
            };
            safe_header(&machine.contract)?;
            if machine.blocks.is_empty() {
                return Err("automatic cleanup has no executable completion body");
            }
            let cycles = crate::control_graph::control_cycle_members(machine)
                .map_err(|_| "cleanup completion has an invalid control graph")?;
            if !cycles.is_empty() && machine.ranked_scc.is_none() {
                return Err("automatic cleanup has an unranked control cycle");
            }
            let mut callees = Vec::new();
            for block in &machine.blocks {
                if matches!(
                    block.terminator,
                    Terminator::Crash { .. }
                        | Terminator::ExternalCompletion { .. }
                        | Terminator::InvokeNoNormal { .. }
                        | Terminator::EliminateEmpty { .. }
                ) {
                    return Err(
                        "automatic cleanup has no proven normal completion on every path",
                    );
                }
                for invocation in block.module_invocations() {
                    let inputs = invocation.inputs;
                    let callee = inputs.callee;
                    if let Some(required) = inputs.required_contract {
                        let header = module
                            .declaration_contract(required)
                            .filter(|header| {
                                header
                                    .selected_application
                                    .as_ref()
                                    .is_some_and(|selected| selected.realization == callee)
                            })
                            .ok_or("cleanup call lost its required public contract")?;
                        safe_header(header)?;
                        if header.termination != semantic_vocabulary::Termination::Terminates {
                            return Err(
                                "cleanup call's required contract promises no termination",
                            );
                        }
                    }
                    callees.push(
                        *positions
                            .get(&callee)
                            .ok_or("cleanup callee body is absent")?,
                    );
                }
                if block.operations.iter().any(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::CallDynamic { .. }
                            | terminal_psi::OperationKind::BoundaryCall { .. }
                    )
                }) {
                    return Err(
                        "automatic cleanup through a dynamic or boundary callable is not served",
                    );
                }
            }
            visited[slot] = 1;
            pending.push((slot, true));
            pending.extend(callees.into_iter().rev().map(|callee| (callee, false)));
        }
    }
    Ok(())
}

/// An owned cleanup is selected by the exact carrier declaration, not by
/// an attached method name or by a special kind of value parameter.
pub fn owned_target<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration>,
    machines: impl Iterator<Item = &'a TerminalMachine>,
    cleanup: &terminal_psi::OwnedCleanupInvocation,
) -> Result<&'a TerminalMachine, &'static str> {
    use terminal_psi::StructuralAccess;
    let declaration = types
        .into_iter()
        .find(|declaration| declaration.id == cleanup.structural_type)
        .ok_or("owned cleanup subject declaration is absent")?;
    let selection = declaration
        .nominal_cleanup
        .as_ref()
        .ok_or("owned cleanup subject has no selected consumer")?;
    if selection.cleanup_machine != cleanup.cleanup_machine
        || selection.declaration.identity() != declaration.identity
        || cleanup.subject.access != StructuralAccess::Owned
    {
        return Err("owned cleanup does not match its retained carrier selection");
    }
    let target = machines
        .into_iter()
        .find(|machine| machine.id == cleanup.cleanup_machine)
        .ok_or("owned cleanup target is absent")?;
    validate_owned_target(declaration, target)?;
    Ok(target)
}

fn validate_owned_target(
    declaration: &StructuralTypeDeclaration,
    target: &TerminalMachine,
) -> Result<(), &'static str> {
    use terminal_psi::StructuralAccess;
    let [parameter] = target.structural_parameters.as_slice() else {
        return Err("owned cleanup needs its one actual value parameter");
    };
    if target.result != TerminalMachineResult::Unit
        || !target.parameters.is_empty()
        || parameter.position != 0
        || parameter.structural_type != declaration.id
        || parameter.access != StructuralAccess::Owned
        || parameter.multiplicity != declaration.multiplicity
    {
        return Err("owned cleanup target has an incompatible value parameter or result");
    }
    Ok(())
}

/// Only the actual selected consumer or the original declaration's package
/// may end the wrapper. Missing package identities never compare equal.
pub fn decomposition_authorized(
    identities: &terminal_psi::TerminalIdentityTable,
    selection: &terminal_psi::NominalCleanupSelection,
    current: semantic_vocabulary::MachineId,
    declaration: Option<&terminal_psi::CheckedMachineDeclaration>,
) -> Result<(), &'static str> {
    if current == selection.cleanup_machine {
        return Ok(());
    }
    let declaration =
        declaration.ok_or("decomposition lost its original machine declaration")?;
    let carrier_owner = crate::closed_conformances::identity_package(
        identities,
        selection.declaration.identity(),
    )?;
    let machine_owner = crate::closed_conformances::identity_package(
        identities,
        declaration.semantic_declaration,
    )?;
    if carrier_owner.is_some() && carrier_owner == machine_owner {
        Ok(())
    } else {
        Err("only the carrier's declaring package or selected cleanup may decompose it")
    }
}

/// Reconstruct the declaration and complete component shape. A caller must
/// separately prove live whole custody, loans and independent claims before
/// committing the event; this static relationship supplies none of them.
pub fn decomposition(
    module: &terminal_psi::TerminalModule,
    current: semantic_vocabulary::MachineId,
    subject: &terminal_psi::StructuralArgument,
) -> Result<semantic_vocabulary::StructuralTypeId, &'static str> {
    use terminal_psi::{StructuralAccess, StructuralPathSegment, StructuralTypeShape};
    if subject.access != StructuralAccess::Owned
        || subject.path.contains(&StructuralPathSegment::Referent)
    {
        return Err("nominal decomposition requires owned storage without a borrow crossing");
    }
    if subject
        .path
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
    {
        return Err("runtime-indexed nominal decomposition is not served");
    }
    let machine = module
        .machine(current)
        .ok_or("decomposition machine is absent")?;
    let root = machine
        .structural_place_type(subject.place)
        .ok_or("decomposition subject has no exact declared type")?;
    let declaration =
        crate::runtime_structural_path_tip(module.structural_types.iter(), root, &subject.path)
            .ok_or("decomposition subject has no exact component shape")?;
    let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        return Err("nominal decomposition of a non-record is not served");
    };
    if fields.iter().any(|field| field.relevance.is_erased()) {
        return Err("nominal decomposition of erased components is not served");
    }
    // Default predicates constrain the established input. Ending the whole
    // wrapper keeps each component's independent qualifications and custody.
    if let Some(selection) = &declaration.nominal_cleanup {
        if selection.declaration.identity() != declaration.identity {
            return Err("decomposition carrier declaration relation is not served");
        }
        decomposition_authorized(
            &module.identities,
            selection,
            current,
            machine.declaration.as_ref(),
        )?;
    }
    Ok(declaration.id)
}
