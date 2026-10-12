//! Original interface tuples and their checked provider completions.
//!
//! Whole provider maps retain the original declaration family. Executable
//! inventory contains complete originals and produced public applications;
//! an unused invocation-generic member needs no invented instantiation.
//! Sibling applications share the selected outer tuple, while real calls must
//! already name a checked boundary plan.
//! Suppliers share the preparation-owned realization correspondence. Neither
//! equal runtime types nor a same-named member establishes either association.

use super::{CheckedTrees, LoweringError, Machine, TraitConformance, UnitPlans, unsupported};
use symbols::SymbolHandle;
use typed_trees::static_applications::{
    InstantiatedRequirementSignature, StaticApplicationIdentity, StaticApplicationView,
};
use typed_trees::trait_definition::AppliedTraitScope;

pub(super) fn closed_requirement(
    checked: &CheckedTrees,
    requirement: SymbolHandle,
) -> Result<Option<&InstantiatedRequirementSignature>, LoweringError> {
    let mut rows = checked
        .typed
        .instantiated_requirement_signatures
        .iter()
        .filter(|(_, row)| {
            checked
                .typed
                .trait_machine_signatures
                .get(row.signature)
                .symbol
                == requirement
        });
    let Some((_, row)) = rows.next() else {
        return Ok(None);
    };
    if rows.next().is_some() {
        return unsupported("public boundary requirement has multiple original applications");
    }
    let application = StaticApplicationView::new(&checked.typed, row.binding_application)
        .map_err(LoweringError::Unsupported)?;
    let selected_interface = if application.application().selected_interface.is_valid() {
        application.application().selected_interface
    } else {
        row.declaring_trait
    };
    if application.declaration().symbol() != row.requirement_template
        || selected_interface != row.selected_interface
        || !checked
            .typed
            .trait_machine_signature(row.requirement_template)
            .is_some_and(|(definition, _)| definition.symbol == row.declaring_trait)
    {
        return unsupported(
            "public boundary requirement lost its original declaration application",
        );
    }
    Ok(Some(row))
}

pub(super) fn retain_complete_interfaces(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    boundaries: &mut Vec<SymbolHandle>,
) -> Result<(), LoweringError> {
    let demanded = boundaries.clone();
    for symbol in demanded {
        super::unique_unit_boundary(plans, symbol)?;
        let Some((definition, _)) = checked.typed.trait_machine_signature(symbol) else {
            continue;
        };
        if !definition.is_boundary {
            continue;
        }
        let application = executable_interface_application(checked, symbol)?;
        retain_interface_applications(checked, plans, &application, boundaries)?;
    }
    Ok(())
}

/// Selection roots own an abstract family, unlike executable call roots.
/// Project that family onto its retained applications before plan discovery.
pub(super) fn selection_boundary_roots(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    roots: &[SymbolHandle],
) -> Result<Vec<SymbolHandle>, LoweringError> {
    let mut boundaries = Vec::new();
    for &symbol in roots {
        let Some((definition, _)) = checked.typed.trait_machine_signature(symbol) else {
            retain_boundary(plans, symbol, &mut boundaries)?;
            continue;
        };
        if !definition.is_boundary {
            return unsupported("selected boundary root belongs to an ordinary contract");
        }
        if closed_requirement(checked, symbol)?.is_some()
            || checked
                .typed
                .closed_requirement_application(symbol)
                .is_some()
        {
            let application = executable_interface_application(checked, symbol)?;
            retain_interface_applications(checked, plans, &application, &mut boundaries)?;
            continue;
        }
        if checked.typed.trait_type_parameters(definition).is_empty()
            && definition.conformance_bounds.is_empty()
            && definition.lifetime_parameters.is_empty()
        {
            retain_interface_applications(
                checked,
                plans,
                &original_interface_application(checked, definition.symbol)?,
                &mut boundaries,
            )?;
        } else {
            // An open outer telescope has no execution tuple of its own.
            // Every actual produced tuple supplies its complete interface;
            // the selected ProviderPlan separately retains the original map.
            for (_, application) in checked
                .typed
                .instantiated_requirement_signatures
                .iter()
                .filter(|(_, row)| row.selected_interface == definition.symbol)
            {
                let symbol = checked
                    .typed
                    .trait_machine_signatures
                    .get(application.signature)
                    .symbol;
                let application = executable_interface_application(checked, symbol)?;
                retain_interface_applications(checked, plans, &application, &mut boundaries)?;
            }
            for application in checked
                .typed
                .closed_requirement_applications()
                .filter(|application| application.declaring_trait == definition.symbol)
            {
                let application =
                    executable_interface_application(checked, application.signature.symbol)?;
                retain_interface_applications(checked, plans, &application, &mut boundaries)?;
            }
        }
    }
    Ok(boundaries)
}

/// Executable requirements retain their issuing interface application. A
/// finite family carries its outer scope separately from the local tuple; a
/// public invocation header carries the complete original binding application.
/// Neither may be reconstructed as an empty declaration scope after planning.
struct InterfaceApplication<'program> {
    scope: AppliedTraitScope<'program>,
    // Public headers retain independent evidence binders in their full key.
    // Finite families instead retain a checked declaring scope; their keys
    // cannot admit evidence outside the declared parameter telescope.
    public_identity: Option<StaticApplicationIdentity>,
}

fn executable_interface_application(
    checked: &CheckedTrees,
    requirement: SymbolHandle,
) -> Result<InterfaceApplication<'_>, LoweringError> {
    if let Some(header) = closed_requirement(checked, requirement)? {
        return Ok(InterfaceApplication {
            scope: closed_interface_scope(checked, header).ok_or(LoweringError::Unsupported(
                "public boundary lost its retained interface scope",
            ))?,
            public_identity: Some(interface_identity(checked, header)?),
        });
    }
    if let Some(application) = checked.typed.closed_requirement_application(requirement) {
        return Ok(InterfaceApplication {
            scope: application
                .trait_scope
                .scope(&checked.typed)
                .filter(|scope| scope.declaration.symbol == application.declaring_trait)
                .ok_or(LoweringError::Unsupported(
                    "finite boundary lost its retained interface scope",
                ))?,
            public_identity: None,
        });
    }
    let (definition, _) =
        checked
            .typed
            .trait_machine_signature(requirement)
            .ok_or(LoweringError::Unsupported(
                "boundary lost its original interface",
            ))?;
    original_interface_application(checked, definition.symbol)
}

fn interface_identity(
    checked: &CheckedTrees,
    row: &InstantiatedRequirementSignature,
) -> Result<StaticApplicationIdentity, LoweringError> {
    typed_trees::static_applications::requirement_interface_application_identity(
        &checked.typed,
        row,
    )
    .map_err(|_| {
        LoweringError::Unsupported("closed boundary interface tuple has no exact identity")
    })
}

fn original_interface_application(
    checked: &CheckedTrees,
    interface: SymbolHandle,
) -> Result<InterfaceApplication<'_>, LoweringError> {
    let scope = original_interface_scope(checked, interface)?;
    let identity = typed_trees::static_applications::static_binding_identity(
        &checked.typed,
        interface,
        &[],
        typed_trees::type_identity::AppliedTypeIdentityOwner::Declaration(interface),
        std::iter::empty::<&typed_trees::name::Identifier>(),
    )
    .map_err(|_| {
        LoweringError::Unsupported("original boundary interface has no exact empty tuple")
    })?;
    Ok(InterfaceApplication {
        scope,
        public_identity: Some(identity),
    })
}

fn original_interface_scope(
    checked: &CheckedTrees,
    interface: SymbolHandle,
) -> Result<AppliedTraitScope<'_>, LoweringError> {
    let declaration =
        checked
            .typed
            .trait_definition_by_symbol(interface)
            .ok_or(LoweringError::Unsupported(
                "original boundary has no interface declaration",
            ))?;
    if !checked.typed.trait_type_parameters(declaration).is_empty()
        || !declaration.conformance_bounds.is_empty()
        || !declaration.lifetime_parameters.is_empty()
    {
        return unsupported("original boundary interface retains an open telescope");
    }
    Ok(AppliedTraitScope::abstract_scope(declaration))
}

fn closed_interface_scope<'program>(
    checked: &'program CheckedTrees,
    header: &InstantiatedRequirementSignature,
) -> Option<AppliedTraitScope<'program>> {
    let declaration = checked
        .typed
        .trait_definition_by_symbol(header.selected_interface)?;
    AppliedTraitScope::from_binding_application(
        &checked.typed,
        declaration,
        header.binding_application,
    )
}

fn retain_interface_applications(
    checked: &CheckedTrees,
    plans: UnitPlans<'_>,
    application: &InterfaceApplication<'_>,
    boundaries: &mut Vec<SymbolHandle>,
) -> Result<(), LoweringError> {
    let scope = &application.scope;
    let selected_interface = scope.declaration.symbol;
    let members = typed_trees::static_applications::boundary_interface_members(
        &checked.typed,
        selected_interface,
    )
    .ok_or(LoweringError::Unsupported(
        "closed boundary has no selected interface declaration",
    ))?;
    for (owner, member) in members {
        let mut retained = false;
        for (_, sibling) in checked
            .typed
            .instantiated_requirement_signatures
            .iter()
            .filter(|(_, row)| {
                row.selected_interface == selected_interface
                    && row.declaring_trait == owner
                    && row.requirement_template == member
            })
        {
            let symbol = checked
                .typed
                .trait_machine_signatures
                .get(sibling.signature)
                .symbol;
            closed_requirement(checked, symbol)?;
            let matches = match &application.public_identity {
                Some(identity) => interface_identity(checked, sibling)? == *identity,
                None => checked
                    .typed
                    .requirement_application_matches_scope(symbol, scope),
            };
            if !matches {
                continue;
            }
            retain_boundary(plans, symbol, boundaries)?;
            retained = true;
        }
        for application in checked
            .typed
            .closed_requirement_applications()
            .filter(|application| {
                application.declaring_trait == owner && application.template == member
            })
        {
            if !checked
                .typed
                .requirement_application_matches_scope(application.signature.symbol, scope)
            {
                continue;
            }
            retain_boundary(plans, application.signature.symbol, boundaries)?;
            retained = true;
        }
        if typed_trees::static_applications::original_requirement_is_complete(
            &checked.typed,
            selected_interface,
            member,
        ) {
            retain_boundary(plans, member, boundaries)?;
            retained = true;
        }
        let (_, signature) =
            checked
                .typed
                .trait_machine_signature(member)
                .ok_or(LoweringError::Unsupported(
                    "boundary interface lost its original member",
                ))?;
        if !retained
            && checked
                .typed
                .state_signature_type_parameters(signature)
                .is_empty()
            && signature.lifetime_parameters.is_empty()
        {
            if std::env::var_os("OMEGA_SOURCE_CONSTANT_INPUT_TRACE").is_some() {
                eprintln!(
                    "TRACE_INTERFACE_COMPLETION selected={selected_interface:?} scope={:?} owner={owner:?} member={member:?}",
                    scope.application_identity(&checked.typed),
                );
                for (_, header) in checked
                    .typed
                    .instantiated_requirement_signatures
                    .iter()
                    .filter(|(_, header)| header.requirement_template == member)
                {
                    let symbol = checked
                        .typed
                        .trait_machine_signatures
                        .get(header.signature)
                        .symbol;
                    eprintln!(
                        "TRACE_INTERFACE_COMPLETION_HEADER symbol={symbol:?} header={header:?} roster={:?} identity={:?}",
                        checked
                            .typed
                            .requirement_interface_applications
                            .span(header.interface_applications),
                        closed_interface_scope(checked, header)
                            .and_then(|scope| scope.application_identity(&checked.typed))
                    );
                }
                for application in checked
                    .typed
                    .closed_requirement_applications()
                    .filter(|application| application.template == member)
                {
                    eprintln!(
                        "TRACE_INTERFACE_COMPLETION_FINITE symbol={:?} declaring={:?} arguments={:?} identity={:?}",
                        application.signature.symbol,
                        application.declaring_trait,
                        application.trait_arguments,
                        application.trait_application_identity
                    );
                }
            }
            return unsupported(
                "closed boundary interface has no checked application for an original member",
            );
        }
    }
    Ok(())
}

fn retain_boundary(
    plans: UnitPlans<'_>,
    symbol: SymbolHandle,
    boundaries: &mut Vec<SymbolHandle>,
) -> Result<(), LoweringError> {
    if !boundaries.contains(&symbol) {
        super::unique_unit_boundary(plans, symbol)?;
        boundaries.push(symbol);
    }
    Ok(())
}

/// Rejoin the exact produced provider and copied conformance. The original
/// edge remains unchanged; preparation retained its template-to-instance pair.
pub(super) fn realizes_closed_requirement(
    checked: &CheckedTrees,
    machine: &Machine,
    provider_template: SymbolHandle,
    named_conformance: arena::Handle<typed_trees::trait_definition::Conformance>,
    application: &InstantiatedRequirementSignature,
) -> Result<bool, LoweringError> {
    let typed = &checked.typed;
    let requirement = typed
        .trait_machine_signatures
        .get(application.signature)
        .symbol;
    let mut rows = typed
        .instantiated_requirement_realizations
        .iter()
        .filter(|(_, row)| {
            row.provider_template == provider_template
                && realization_contains_candidate(
                    typed,
                    row.provider_template,
                    row.provider_instance,
                    machine.symbol,
                )
                && row.named_conformance == named_conformance
                && row.requirement_template == application.requirement_template
                && row.requirement_instance == requirement
        });
    let Some((_, row)) = rows.next() else {
        return Ok(false);
    };
    if rows.next().is_some() {
        return unsupported("closed provider repeats its exact requirement realization");
    }
    let template = typed
        .machine(row.provider_template)
        .ok_or(LoweringError::Unsupported(
            "closed provider has no original machine",
        ))?;
    if named_conformance.is_valid() {
        if row.conformance.is_valid() || row.instance_conformance.is_valid() {
            return unsupported("closed named provider also carries a machine conformance source");
        }
        let declaration = typed
            .conformances
            .iter()
            .find(|(handle, _)| *handle == named_conformance)
            .map(|(_, declaration)| declaration)
            .ok_or(LoweringError::Unsupported(
                "closed named provider lost its original map",
            ))?;
        let mut mappings = declaration.rows.iter().filter(|mapping| {
            mapping.declaring_trait == application.declaring_trait
                && mapping.requirement == application.requirement_template
        });
        if !mappings.next().is_some_and(|mapping| {
            mapping.realization_machine == provider_template
                && mapping.realization_state == row.realization_state
                && typed
                    .machine_states(template)
                    .first()
                    .is_some_and(|entry| entry.symbol == mapping.realization_state)
        }) || mappings.next().is_some()
        {
            return unsupported("closed named provider lost its exact original realization row");
        }
    } else {
        if row.realization_state.is_valid() {
            return unsupported("closed machine provider also carries a named realization source");
        }
        let original = typed
            .machine_trait_conformances(template)
            .iter()
            .enumerate()
            .find(|(ordinal, _)| template.satisfies.handle_at(*ordinal as u32) == row.conformance)
            .map(|(_, edge)| edge)
            .ok_or(LoweringError::Unsupported(
                "closed provider lost its original conformance",
            ))?;
        let produced = typed
            .machine_trait_conformances(machine)
            .iter()
            .enumerate()
            .find(|(ordinal, _)| {
                machine.satisfies.handle_at(*ordinal as u32) == row.instance_conformance
            })
            .map(|(_, edge)| edge)
            .ok_or(LoweringError::Unsupported(
                "closed provider lost its copied conformance",
            ))?;
        if original != produced || original.requirement_symbol != application.requirement_template {
            return unsupported("closed provider changed its original requirement edge");
        }
    }
    Ok(true)
}

/// An original requirement can be complete without a public application
/// header. Its physical candidate still owns either a direct specialization
/// receipt or one exact source/produced family correspondence. Recover that
/// owner from retained handles, not from its types or diagnostic path.
pub(super) fn realization_candidate_matches(
    typed: &typed_trees::TypedTrees,
    template: SymbolHandle,
    candidate: SymbolHandle,
) -> bool {
    if exact_specialization(typed, template, candidate) {
        return true;
    }
    let mut families = typed.target_machine_families.iter().filter(|family| {
        family
            .members
            .iter()
            .any(|member| member.machine == candidate)
            && realization_contains_candidate(typed, template, family.canonical, candidate)
    });
    families.next().is_some() && families.next().is_none()
}

/// Project a preparation-owned source realization onto its actual body roster.
/// A portable source receipt continues to name its head instance. The family
/// producer closed the head and every member from that same complete tuple;
/// this reader rejoins those retained handles, never selects a target in Psi.
pub(super) fn realization_contains_candidate(
    typed: &typed_trees::TypedTrees,
    template: SymbolHandle,
    instance: SymbolHandle,
    candidate: SymbolHandle,
) -> bool {
    if !exact_specialization(typed, template, instance) {
        return false;
    }
    if instance == candidate {
        return true;
    }
    let mut sources = typed
        .target_machine_families
        .iter()
        .filter(|family| family.canonical == template);
    let Some(source) = sources.next() else {
        return false;
    };
    let mut instances = typed
        .target_machine_families
        .iter()
        .filter(|family| family.canonical == instance);
    let Some(produced) = instances.next() else {
        return false;
    };
    if sources.next().is_some()
        || instances.next().is_some()
        || source.members.len() != produced.members.len()
    {
        return false;
    }
    let mut matching = produced
        .members
        .iter()
        .filter(|member| member.machine == candidate);
    let Some(member) = matching.next() else {
        return false;
    };
    if matching.next().is_some() {
        return false;
    }
    // The closed family roster is an existing producer receipt. Each target
    // owns exactly one original and one produced member, joined by its exact
    // specialization. A member from another closed tuple cannot enter here.
    source.members.iter().all(|original| {
        let mut originals = source
            .members
            .iter()
            .filter(|row| row.target == original.target);
        let mut closed_members = produced
            .members
            .iter()
            .filter(|row| row.target == original.target);
        let Some(first_original) = originals.next() else {
            return false;
        };
        let Some(closed) = closed_members.next() else {
            return false;
        };
        originals.next().is_none()
            && closed_members.next().is_none()
            && exact_specialization(typed, first_original.machine, closed.machine)
    }) && source.members.iter().any(|original| {
        original.target == member.target && exact_specialization(typed, original.machine, candidate)
    })
}

fn exact_specialization(
    typed: &typed_trees::TypedTrees,
    template: SymbolHandle,
    instance: SymbolHandle,
) -> bool {
    if template == instance {
        return typed.machine(template).is_some();
    }
    let mut receipts = typed
        .machine_specializations
        .iter()
        .filter(|receipt| receipt.instance == instance);
    receipts.next().is_some_and(|receipt| {
        receipt.template == template && receipt.binding_application.is_valid()
    }) && receipts.next().is_none()
}

pub(super) fn satisfies_closed_requirement(
    checked: &CheckedTrees,
    machine: &Machine,
    conformance: &TraitConformance,
    application: &InstantiatedRequirementSignature,
) -> Result<bool, LoweringError> {
    let requirement = checked
        .typed
        .trait_machine_signatures
        .get(application.signature)
        .symbol;
    for (_, row) in checked
        .typed
        .instantiated_requirement_realizations
        .iter()
        .filter(|(_, row)| {
            row.provider_instance == machine.symbol && row.requirement_instance == requirement
        })
    {
        let Some((ordinal, _)) = checked
            .typed
            .machine_trait_conformances(machine)
            .iter()
            .enumerate()
            .find(|(_, edge)| std::ptr::eq(*edge, conformance))
        else {
            return unsupported("closed provider conformance is outside its original roster");
        };
        if machine.satisfies.handle_at(ordinal as u32) == row.instance_conformance
            && realizes_closed_requirement(
                checked,
                machine,
                row.provider_template,
                Default::default(),
                application,
            )?
        {
            return Ok(true);
        }
    }
    Ok(false)
}
