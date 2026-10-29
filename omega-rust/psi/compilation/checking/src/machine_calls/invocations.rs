use super::effect_inference::full_parameter_invocation;
use diagnostics::Diagnostic;
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;

pub(crate) fn validate_invocation_contracts(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_declared_targets(program, diagnostics);
    let plan = crate::infer_synchronous_invocations(program);
    let mut requirements = Vec::new();

    for machine in program.machines() {
        let Some(summary) = plan.for_machine(machine.symbol) else {
            continue;
        };
        let publishes = machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
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
        collect_machine_requirements(program, machine, &mut requirements);
    }
    for conformance in program.conformances() {
        collect_named_requirements(conformance, &mut requirements);
    }
    validate_requirement_refinements(program, &plan, &requirements, diagnostics);
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct RequirementInvocation {
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
}

fn retain_requirement(
    requirements: &mut Vec<RequirementInvocation>,
    target: RequirementInvocation,
) {
    if !requirements.contains(&target) {
        requirements.push(target);
    }
}

fn collect_named_requirements(
    conformance: &typed_trees::trait_definition::Conformance,
    requirements: &mut Vec<RequirementInvocation>,
) {
    for row in &conformance.rows {
        retain_requirement(
            requirements,
            RequirementInvocation {
                machine: row.realization_machine,
                state: row.realization_state,
                requirement: row.requirement,
            },
        );
    }
}

/// Semantic revalidation uses the same exact row correspondence as normal
/// checking. The caller prepares one invocation plan for the entire judgment;
/// a row never infers a replacement promise from its receiving contract.
pub(crate) fn validate_named_conformance_refinement(
    program: &dyn crate::CheckingInput,
    plan: &flow_effects::InvocationInferencePlan,
    conformance: &typed_trees::trait_definition::Conformance,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut requirements = Vec::new();
    collect_named_requirements(conformance, &mut requirements);
    for target in &requirements {
        if let Some(machine) = program.machine(target.machine)
            && let Some(state) = program.state_in_machine(machine, target.state)
        {
            validate_target_names(
                program,
                machine.name.as_str(),
                program.machine_invokes(machine),
                program.state_parameters(state),
                diagnostics,
            );
        }
        if let Some((definition, requirement)) = program.trait_machine_signature(target.requirement)
        {
            validate_target_names(
                program,
                &format!("{}::{}", definition.name, requirement.name),
                program.state_signature_invokes(requirement),
                program.state_signature_parameters(requirement),
                diagnostics,
            );
        }
    }
    validate_requirement_refinements(program, plan, &requirements, diagnostics);
}

fn validate_declared_targets(
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
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
    program: &dyn crate::CheckingInput,
    owner: &str,
    targets: &[typed_trees::signature::AuthoredInvocation],
    parameters: &[StateParameter],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = Vec::new();
    for target in targets {
        if target.target != typed_trees::signature::AuthoredInvocationTarget::Unresolved
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
            typed_trees::signature::AuthoredInvocationTarget::Unresolved => {
                diagnostics.push(Diagnostic::error(format!(
                    "callable `{owner}` declares `invokes {};`, but `{}` is neither one of its boundary-binding parameters nor a known boundary trait",
                    target.name,
                    target.name,
                )));
            }
            typed_trees::signature::AuthoredInvocationTarget::Parameter { ordinal, symbol } => {
                let parameter = parameters.iter().nth(ordinal as usize);
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
            typed_trees::signature::AuthoredInvocationTarget::Service(symbol) => {
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

fn collect_machine_requirements(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    requirements: &mut Vec<RequirementInvocation>,
) {
    let Some(state) = program.machine_states(machine).first() else {
        return;
    };
    for conformance in program.machine_trait_conformances(machine) {
        let Some(definition) = program.trait_definition_by_symbol(conformance.symbol) else {
            continue;
        };
        // Legacy machine-level satisfaction retains its authored member name.
        // Named maps below already own exact requirement/state coordinates.
        for requirement in program.trait_machine_signatures(definition) {
            let matches = conformance.requirement.as_ref().map_or_else(
                || {
                    machine
                        .name
                        .as_str()
                        .rsplit("::")
                        .next()
                        .is_some_and(|name| name == requirement.name.as_str())
                },
                |name| name.as_str() == requirement.name.as_str(),
            );
            if matches {
                retain_requirement(
                    requirements,
                    RequirementInvocation {
                        machine: machine.symbol,
                        state: state.symbol,
                        requirement: requirement.symbol,
                    },
                );
            }
        }
    }
}

fn validate_requirement_refinements(
    program: &dyn crate::CheckingInput,
    plan: &flow_effects::InvocationInferencePlan,
    requirements: &[RequirementInvocation],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for target in requirements {
        let Some((machine, state)) = program.machine(target.machine).and_then(|machine| {
            program
                .state_in_machine(machine, target.state)
                .map(|state| (machine, state))
        }) else {
            diagnostics.push(Diagnostic::error(
                "invocation conformance retains no exact realization machine and state",
            ));
            continue;
        };
        let Some((trait_definition, requirement)) =
            program.trait_machine_signature(target.requirement)
        else {
            diagnostics.push(Diagnostic::error(
                "invocation conformance retains no exact declaring trait and requirement",
            ));
            continue;
        };
        let Some(summary) = plan.for_machine(machine.symbol) else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` conformance has no inferred synchronous-invocation summary",
                machine.name,
            )));
            continue;
        };
        let parameters = program.state_signature_parameters(requirement);
        let allowed = crate::declared_signature_invocations(program, requirement)
            .into_iter()
            .map(|target| full_parameter_invocation(target, parameters))
            .collect::<Option<Vec<_>>>();
        let actual_parameters = program.state_parameters(state);
        let self_forwarded = trait_definition.is_boundary
            && crate::has_self_forwarded_boundary_parameter(
                program,
                machine,
                trait_definition.symbol,
                parameters.iter().count(),
            );
        let actual = summary
            .inferred_transitive
            .iter()
            .filter_map(|target| match *target {
                flow_effects::InvocationTarget::Parameter(0) if self_forwarded => None,
                flow_effects::InvocationTarget::Parameter(index) if self_forwarded => {
                    Some(flow_effects::InvocationTarget::Parameter(index - 1))
                }
                target => Some(target),
            })
            .map(|target| {
                // The boundary adapter's extra receiver was removed above,
                // so its remaining ordinals already name requirement
                // parameters. Ordinary satisfiers retain their own roster.
                full_parameter_invocation(
                    target,
                    if self_forwarded {
                        parameters
                    } else {
                        actual_parameters
                    },
                )
            })
            .collect::<Option<Vec<_>>>();
        let (Some(actual), Some(allowed)) = (actual, allowed) else {
            diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` conformance to `{}::{}` retains an invocation outside its declared parameter roster",
                    machine.name, trait_definition.name, requirement.name,
                )));
            continue;
        };
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

fn report_missing_targets(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    actual: &[flow_effects::InvocationTarget],
    allowed: &[flow_effects::InvocationTarget],
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
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    target: flow_effects::InvocationTarget,
) -> String {
    match target {
        flow_effects::InvocationTarget::Parameter(index) => parameters
            .iter()
            .nth(index as usize)
            .map(|parameter| parameter.name.as_str().to_owned())
            .unwrap_or_else(|| format!("parameter#{index}")),
        flow_effects::InvocationTarget::Service(symbol) => program
            .traits()
            .iter()
            .find(|definition| definition.is_boundary && definition.symbol == symbol)
            .map(|definition| definition.name.as_str().to_owned())
            .unwrap_or_else(|| format!("service#{}", symbol.arena_index())),
    }
}

/// The boundary requirement symbol a type's service carrier names — or the
/// type's own symbol when no exact service-parameter carrier classifies it.
/// Callers then admit the symbol against the boundary trait table their own
/// way (`.find` for the definition, `.any().then_some` for the symbol).
pub(crate) fn boundary_requirement_symbol(
    program: &dyn crate::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> symbols::SymbolHandle {
    typed_trees::invocations::boundary_requirement_symbol(program.typed_program(), type_reference)
}

fn boundary_trait_for_type(
    program: &dyn crate::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<&typed_trees::trait_definition::TraitDefinition> {
    let symbol = boundary_requirement_symbol(program, type_reference);
    program
        .traits()
        .iter()
        .find(|definition| definition.is_boundary && definition.symbol == symbol)
}
