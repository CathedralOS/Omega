//! Freeze callable descriptions before product pruning can remove their bodies.
//!
//! This is the identity projection formerly reconstructed by stage05. Only
//! declaration coordinates and checked commitments survive, not source bodies.
//! Checking owns this publication; later consumers read the frozen inventory
//! rather than requesting a source-signature refresh.

use checked_trees::{
    CheckedCallableHeader, CheckedDeclarationCapture, CheckedDeclarationDescription,
    CheckedDeclarationDescriptions, CheckedDeclarationParameter, CheckedDeclarationSpecialization,
    CheckedMachineDescription, CheckedSynchronousBoundaryTarget, CheckedTrees,
};
use symbols::SymbolHandle;

pub(crate) fn retain_declaration_descriptions(checked: &mut CheckedTrees) {
    let mut descriptions = CheckedDeclarationDescriptions {
        selections: validation::checked_selection_requirements(checked),
        evaluation_dependencies: validation::compiler_evaluation::checked_dependencies(checked),
        ..Default::default()
    };
    let sidecars = typed_trees::references::MachineSidecarIndex::build(&checked.typed);
    // Invocation summaries are keyed by exact machine, not iteration position.
    // Sort borrowed rows once rather than scanning the whole table per header.
    let mut summaries = checked
        .facts
        .synchronous_invocations
        .machines
        .iter()
        .collect::<Vec<_>>();
    summaries.sort_by_key(|summary| (summary.machine.arena_index(), summary.machine.generation()));
    for machine in checked.typed.machines() {
        let synchronous_invocations = synchronous_invocations(checked, machine, &summaries)
            .map(|targets| descriptions.invocation_targets.insert_many(targets));
        // Do not put this behind `describe`: an open or otherwise unavailable
        // signature does not erase its independently checked supply mode.
        descriptions.headers.insert(CheckedCallableHeader {
            machine: machine.symbol,
            callable: checked.typed.machine_callable_identity(machine),
            package: checked.symbols.symbol_package_identity(machine.symbol),
            source_target: checked.source_target_context(machine).map(str::to_owned),
            product_eligible: !machine.target_family_interface
                && checked.symbols.symbol_is_in_dependency_scope(
                    machine.symbol,
                    source::DependencyScope::Product,
                ),
            supplies_checked_body: machine.supplies_checked_body(),
            type_parameter_count: checked.typed.machine_type_parameters(machine).len(),
            compiler_evaluation: validation::compiler_evaluation::checked_declaration(
                checked,
                machine,
                &descriptions.selections,
                &sidecars,
            ),
            synchronous_invocations,
        });
        let row = match describe(checked, &mut descriptions, machine.symbol) {
            Ok(description) => CheckedDeclarationDescription::Machine(description),
            Err(reason) => CheckedDeclarationDescription::Unavailable {
                machine: machine.symbol,
                reason,
            },
        };
        descriptions.machines.insert(row);
    }
    // Preserve any previously retained dormant declaration. A missing body is
    // not authority to invent a replacement description or erase its identity.
    let previous = &checked.facts.declaration_descriptions;
    for (_, header) in previous.headers.iter() {
        if checked.machine(header.machine).is_none() {
            let mut header = header.clone();
            header.synchronous_invocations = header.synchronous_invocations.and_then(|span| {
                previous
                    .invocation_targets
                    .span(span)
                    .map(|targets| {
                        descriptions
                            .invocation_targets
                            .insert_many(targets.iter().cloned())
                    })
                    .ok_or_else(|| {
                        "checked declaration lost its retained invocation targets".to_owned()
                    })
            });
            descriptions.headers.insert(header);
        }
    }
    for (_, row) in previous.machines.iter() {
        if checked.machine(row.machine()).is_some() {
            continue;
        }
        let mut row = row.clone();
        if let CheckedDeclarationDescription::Machine(description) = &mut row {
            let Some(parameters) = previous.parameters.span(description.parameters) else {
                descriptions
                    .machines
                    .insert(CheckedDeclarationDescription::Unavailable {
                        machine: description.machine,
                        reason: "checked declaration lost its retained parameter identities".into(),
                    });
                continue;
            };
            description.parameters = descriptions
                .parameters
                .insert_many(parameters.iter().cloned());
        }
        descriptions.machines.insert(row);
    }
    descriptions.index_machines();
    checked.facts.declaration_descriptions = descriptions;
}

fn synchronous_invocations(
    checked: &CheckedTrees,
    machine: &typed_trees::machine::Machine,
    summaries: &[&checked_trees::MachineSynchronousInvocationFact],
) -> Result<Vec<CheckedSynchronousBoundaryTarget>, String> {
    let key = (machine.symbol.arena_index(), machine.symbol.generation());
    let start = summaries.partition_point(|summary| {
        (summary.machine.arena_index(), summary.machine.generation()) < key
    });
    let end = summaries.partition_point(|summary| {
        (summary.machine.arena_index(), summary.machine.generation()) <= key
    });
    let [summary] = &summaries[start..end] else {
        return Err(format!(
            "checked adapter `{}` resolves to {} exact checked synchronous-invocation facts",
            machine.name,
            end - start
        ));
    };
    summary
        .realized_targets
        .iter()
        .map(|target| synchronous_target(checked, machine, *target))
        .collect()
}

fn synchronous_target(
    checked: &CheckedTrees,
    machine: &typed_trees::machine::Machine,
    target: flow_effects::RealizedInvocationTarget,
) -> Result<CheckedSynchronousBoundaryTarget, String> {
    let typed = &checked.typed;
    let (symbol, application) = match target {
        flow_effects::RealizedInvocationTarget::Requirement(requirement) => {
            let scope = if let Some(application) = typed.closed_requirement_application(requirement) {
                application.trait_scope.scope(typed)
            } else {
                typed.trait_machine_signature(requirement).and_then(|(declaration, _)| {
                    (typed.trait_type_parameters(declaration).is_empty() && declaration.lifetime_parameters.is_empty())
                        .then(|| typed_trees::trait_definition::AppliedTraitScope::abstract_scope(declaration))
                })
            }.ok_or_else(|| format!("checked adapter `{}` synchronous invocation lost its exact requirement application", machine.name))?;
            let application = scope.portable_application_identity(typed)
                .and_then(|identity| typed.type_identity_text(identity))
                .ok_or_else(|| format!("checked adapter `{}` synchronous invocation has no portable application identity", machine.name))?;
            (scope.declaration.symbol, Some(application))
        }
        flow_effects::RealizedInvocationTarget::Abstract(
            flow_effects::InvocationTarget::Service(symbol),
        ) => (symbol, None),
        flow_effects::RealizedInvocationTarget::Abstract(
            flow_effects::InvocationTarget::Parameter(parameter),
        ) => {
            return Err(format!(
                "checked adapter `{}` retains retired runtime boundary-binding parameter {parameter} in its synchronous invocation graph",
                machine.name
            ));
        }
    };
    if !symbol.is_valid() {
        return Err(format!(
            "checked adapter `{}` has an invalid exact synchronous-invocation service symbol",
            machine.name
        ));
    }
    if !typed
        .symbols
        .symbol_is_in_dependency_scope(symbol, source::DependencyScope::Product)
    {
        return Err(format!(
            "checked adapter `{}` synchronous-invocation service {:?} is outside the Product planning context",
            machine.name, symbol
        ));
    }
    let definition = typed.trait_definition_by_symbol(symbol).filter(|definition| definition.is_boundary)
        .ok_or_else(|| format!("checked adapter `{}` synchronous-invocation symbol {:?} has no exact boundary trait", machine.name, symbol))?;
    Ok(CheckedSynchronousBoundaryTarget {
        declaration: typed.trait_declaration_path(definition),
        package: typed.symbols.symbol_package_identity(symbol),
        application,
    })
}

fn describe(
    checked: &CheckedTrees,
    descriptions: &mut CheckedDeclarationDescriptions,
    machine: SymbolHandle,
) -> Result<CheckedMachineDescription, String> {
    let declaration = checked
        .machine(machine)
        .ok_or("checked machine declaration is absent")?;
    let entry = checked
        .machine_states(declaration)
        .first()
        .ok_or("checked machine declaration has no entry state")?;
    let callable = checked.typed.machine_callable_identity(declaration);
    if !callable.is_valid() {
        return Err("checked machine has no callable signature identity".into());
    }
    let contract = checked
        .facts
        .contract_plans
        .for_machine(machine)
        .ok_or("checked machine declaration has no contract")?
        .commitment;
    let specialization = checked
        .typed
        .executable_application_template(machine)?
        .map(|template| {
            let identity = checked
                .facts
                .specialization_identities
                .for_instance(machine)
                .ok_or("checked executable application has no retained identity")?;
            let callable = checked
                .machine(template)
                .map(|template| checked.typed.machine_callable_identity(template))
                .filter(|identity| identity.is_valid())
                .ok_or("checked specialization lost its exact template callable")?;
            Ok::<_, String>(CheckedDeclarationSpecialization {
                template,
                callable,
                commitment: identity.commitment.as_bytes(),
                capture: capture(checked, machine)?,
            })
        })
        .transpose()?;
    let parameters =
        descriptions
            .parameters
            .insert_many(checked.state_parameters(entry).iter().map(|parameter| {
                CheckedDeclarationParameter {
                    normalized: checked.normalized_type_identity(parameter.type_reference),
                    qualified: checked.package_qualified_type_identity(parameter.type_reference),
                }
            }));
    Ok(CheckedMachineDescription {
        machine,
        entry: entry.symbol,
        parameters,
        result: checked.package_qualified_type_identity(entry.return_type),
        callable,
        contract,
        specialization,
    })
}

fn capture(
    checked: &CheckedTrees,
    machine: SymbolHandle,
) -> Result<Option<CheckedDeclarationCapture>, String> {
    let template = checked.executable_application_template(machine)?;
    let mut direct = checked
        .captured_machine_applications
        .iter()
        .filter(|(_, capture)| capture.instance == machine)
        .map(|(_, capture)| capture);
    let direct_capture = direct.next();
    if direct.next().is_some() {
        return Err("checked declaration has ambiguous capture ownership".into());
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
            return Err("checked invocation has ambiguous capture ownership".into());
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
            let template_identity = checked
                .type_identities
                .intern(&capture.normalized_template_identity);
            Ok(CheckedDeclarationCapture {
                template_identity,
                template_contract: receipt.template_contract_commitment.as_bytes(),
                specialization: receipt.commitment.as_bytes(),
            })
        })
        .transpose()
}
