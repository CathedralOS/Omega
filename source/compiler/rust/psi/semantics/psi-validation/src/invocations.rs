use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::StateParameter;

pub(crate) fn validate_invocation_contracts(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_declared_targets(program, diagnostics);
    let plan = psi_effects::infer_synchronous_invocations(program);

    for machine in program.machines() {
        let Some(summary) = plan.for_machine(machine.symbol) else {
            continue;
        };
        let publishes = machine.supply_mode
            != psi_language_semantics::MachineSupplyMode::CheckedBody
            || machine.is_public
            || !program.machine_invokes(machine).is_empty();
        if publishes {
            let parameters = program
                .machine_states(machine)
                .first()
                .map(|state| program.state_parameters(state))
                .unwrap_or_default();
            report_missing_targets(
                program,
                parameters,
                &summary.inferred_transitive,
                &summary.published,
                |binding, ceiling| {
                    format!(
                        "machine `{}` omits `invokes {binding};` from its published synchronous-invocation ceiling, but its body may invoke that binding before returning (published invokes: [{ceiling}])",
                        machine.name,
                    )
                },
                diagnostics,
            );
        }
        validate_conformance_refinement(program, machine, summary, diagnostics);
    }
}

fn validate_declared_targets(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            validate_target_names(
                program,
                &format!("{}::{}", trait_definition.name, signature.name),
                program.state_signature_invokes(signature),
                program.state_signature_parameters(signature),
                diagnostics,
            );
        }
    }
    for machine in program.machines() {
        let parameters = program
            .machine_states(machine)
            .first()
            .map(|state| program.state_parameters(state))
            .unwrap_or_default();
        validate_target_names(
            program,
            machine.name.as_str(),
            program.machine_invokes(machine),
            parameters,
            diagnostics,
        );
    }
}

fn validate_target_names(
    program: &TypedTrees,
    owner: &str,
    targets: &[psi_typed_trees::signature::AuthoredInvocation],
    parameters: &[StateParameter],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = Vec::new();
    for target in targets {
        if target.target != psi_typed_trees::signature::AuthoredInvocationTarget::Unresolved
            && seen.contains(&target.target)
        {
            diagnostics.push(Diagnostic::error(format!(
                "callable `{owner}` declares `invokes {};` more than once",
                target.name,
            )));
            continue;
        }
        seen.push(target.target);

        match target.target {
            psi_typed_trees::signature::AuthoredInvocationTarget::Unresolved => {
                diagnostics.push(Diagnostic::error(format!(
                    "callable `{owner}` declares `invokes {};`, but `{}` is neither one of its boundary-binding parameters nor a known boundary trait",
                    target.name,
                    target.name,
                )));
            }
            psi_typed_trees::signature::AuthoredInvocationTarget::Parameter { ordinal, symbol } => {
                let parameter = parameters
                    .iter()
                    .filter(|parameter| !parameter.is_self)
                    .nth(ordinal as usize);
                if parameter.is_none_or(|parameter| {
                    parameter.symbol != symbol
                        || boundary_trait_for_type(program, parameter.type_reference).is_none()
                }) {
                    diagnostics.push(Diagnostic::error(format!(
                        "callable `{owner}` retains `invokes {};` with a stale or non-boundary parameter target",
                        target.name,
                    )));
                }
            }
            psi_typed_trees::signature::AuthoredInvocationTarget::Service(symbol) => {
                if !program
                    .traits()
                    .iter()
                    .any(|definition| definition.symbol == symbol && definition.is_boundary)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "callable `{owner}` retains `invokes {};` with a stale or non-boundary service target",
                        target.name,
                    )));
                }
            }
        }
    }
}

fn validate_conformance_refinement(
    program: &TypedTrees,
    machine: &Machine,
    summary: &psi_effects::MachineInvocationInference,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for conformance in program.machine_trait_conformances(machine) {
        let Some(trait_definition) = program
            .traits()
            .iter()
            .find(|definition| definition.symbol == conformance.symbol)
        else {
            continue;
        };
        let requirements = program.trait_machine_signatures(trait_definition);
        let matching = requirements
            .iter()
            .filter(|requirement| {
                conformance
                    .requirement
                    .as_ref()
                    .map(|name| name.as_str() == requirement.name.as_str())
                    .unwrap_or_else(|| {
                        machine
                            .name
                            .as_str()
                            .rsplit("::")
                            .next()
                            .is_some_and(|name| name == requirement.name.as_str())
                    })
            })
            .collect::<Vec<_>>();
        for requirement in matching {
            let allowed = psi_effects::declared_signature_invocations(program, requirement);
            let parameters = program.state_signature_parameters(requirement);
            let self_forwarded = trait_definition.is_boundary
                && psi_effects::has_self_forwarded_boundary_parameter(
                    program,
                    machine,
                    trait_definition.symbol,
                    parameters
                        .iter()
                        .filter(|parameter| !parameter.is_self)
                        .count(),
                );
            let actual = summary
                .inferred_transitive
                .iter()
                .filter_map(|target| match *target {
                    psi_effects::InvocationTarget::Parameter(0) if self_forwarded => None,
                    psi_effects::InvocationTarget::Parameter(index) if self_forwarded => {
                        Some(psi_effects::InvocationTarget::Parameter(index - 1))
                    }
                    target => Some(target),
                })
                .collect::<Vec<_>>();
            report_missing_targets(
                program,
                parameters,
                &actual,
                &allowed,
                |binding, ceiling| {
                    format!(
                        "machine `{}` does not refine `{}::{}`: its body may synchronously invoke binding `{binding}`, but the requirement omits `invokes {binding};` (requirement invokes: [{ceiling}])",
                        machine.name, trait_definition.name, requirement.name,
                    )
                },
                diagnostics,
            );
        }
    }
}

fn report_missing_targets(
    program: &TypedTrees,
    parameters: &[StateParameter],
    actual: &[psi_effects::InvocationTarget],
    allowed: &[psi_effects::InvocationTarget],
    message: impl Fn(&str, &str) -> String,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ceiling = allowed
        .iter()
        .map(|target| invocation_target_label(program, parameters, *target))
        .collect::<Vec<_>>()
        .join(", ");
    for target in actual {
        if allowed.contains(target) {
            continue;
        }
        let binding = invocation_target_label(program, parameters, *target);
        diagnostics.push(Diagnostic::error(message(&binding, &ceiling)));
    }
}

fn invocation_target_label(
    program: &TypedTrees,
    parameters: &[StateParameter],
    target: psi_effects::InvocationTarget,
) -> String {
    match target {
        psi_effects::InvocationTarget::Parameter(index) => parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .nth(index as usize)
            .map(|parameter| parameter.name.as_str().to_owned())
            .unwrap_or_else(|| format!("parameter#{index}")),
        psi_effects::InvocationTarget::Service(symbol) => program
            .traits()
            .iter()
            .find(|definition| definition.is_boundary && definition.symbol == symbol)
            .map(|definition| definition.name.as_str().to_owned())
            .unwrap_or_else(|| format!("service#{}", symbol.arena_index())),
    }
}

fn boundary_trait_for_type(
    program: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Option<&psi_typed_trees::trait_definition::TraitDefinition> {
    let symbol = program
        .type_reference_table
        .type_reference(type_reference)
        .type_symbol(&program.type_reference_table);
    program
        .traits()
        .iter()
        .find(|definition| definition.is_boundary && definition.symbol == symbol)
}
