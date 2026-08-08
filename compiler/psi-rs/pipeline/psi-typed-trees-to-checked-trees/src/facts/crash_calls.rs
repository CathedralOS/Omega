use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;

use super::{encode_contract_fact_canonical, encode_expression_canonical, is_true_crash_route};

/// Materialize direct invocation-specific crash refinement while the typed
/// expressions are still available. Selection uses a published ceiling when
/// one exists and a conservative monotone checked-body summary for same-unit
/// private machines. Recursive components close over their finite cause/scope
/// buckets; a dependency outside the recognized local/capsule graph keeps its
/// caller unexamined. The retained rows are entirely checked data: downstream
/// propagation can distinguish a proved-crash-free call from an unexamined call
/// without reopening source trees.
pub(super) fn attach_checked_crash_calls(
    program: &TypedTrees,
    flow: &psi_checked_trees::FlowFacts,
    content_conservation: &[psi_validation::ContentConservationSourcePlan],
    crash_capsules: &[psi_checked_trees::CrashContractCapsule],
    plans: &mut [psi_checked_trees::MachineContractPlan],
) {
    let inferred_body_summaries =
        infer_private_body_summaries(program, flow, crash_capsules, plans);
    let mut calls_by_caller =
        Vec::<(SymbolHandle, Vec<psi_checked_trees::CheckedCrashCallSite>)>::new();
    for (_, state_flow) in flow.control.states.iter() {
        let caller_parameter_names = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == state_flow.machine_symbol)
            .and_then(|machine| program.machine_states(machine).first())
            .map(|entry| {
                program
                    .state_parameters(entry)
                    .iter()
                    .map(|parameter| parameter.name.as_str().to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for call_flow in flow.control.calls.span_or_empty(state_flow.calls) {
            let Some((target_machine_symbol, target_state_symbol)) =
                crate::contract_target_from_state_symbol(program, call_flow.target_symbol)
            else {
                continue;
            };
            let local_target = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == target_machine_symbol);
            let local_plan = plans
                .iter()
                .find(|plan| plan.machine == target_machine_symbol);
            let (
                target_parameters,
                target_parameter_names,
                target_buckets,
                route_contracts,
                uses_published_routes,
                target_contract_fingerprint,
            ) = if let (Some(target_machine), Some(target_plan)) = (local_target, local_plan) {
                let Some(target_state) = program
                    .machine_states(target_machine)
                    .iter()
                    .find(|state| state.symbol == target_state_symbol)
                else {
                    continue;
                };
                let target_buckets = if !target_plan.crash.published().is_empty() {
                    target_plan.crash.published().to_vec()
                } else if target_machine.supply_mode
                    == psi_language_semantics::MachineSupplyMode::CheckedBody
                {
                    let Some((_, summary)) = inferred_body_summaries
                        .iter()
                        .find(|(machine, _)| *machine == target_machine_symbol)
                    else {
                        // Dependency-unresolved private bodies remain
                        // unexamined rather than erasing a nested crash.
                        continue;
                    };
                    summary.clone()
                } else {
                    // Omission on a requirement/boundary/exported interface is
                    // the published negative guarantee, so retain an empty row
                    // as positive crash-free evidence.
                    Vec::new()
                };
                let uses_published_routes = !target_plan.crash.published().is_empty();
                (
                    program.state_parameters(target_state),
                    program
                        .machine_states(target_machine)
                        .first()
                        .map(|entry| {
                            program
                                .state_parameters(entry)
                                .iter()
                                .map(|parameter| parameter.name.as_str().to_owned())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default(),
                    target_buckets,
                    uses_published_routes.then(|| program.machine_contracts(target_machine)),
                    uses_published_routes,
                    target_plan.fingerprint,
                )
            } else {
                let Some(capsule) = crash_capsules.iter().find(|capsule| {
                    capsule.target_machine() == target_machine_symbol
                        && capsule.target_state() == target_state_symbol
                }) else {
                    continue;
                };
                let Some(signature) =
                    requirement_signature(program, target_machine_symbol, target_state_symbol)
                else {
                    continue;
                };
                let parameters = program.state_signature_parameters(signature);
                (
                    parameters,
                    parameters
                        .iter()
                        .map(|parameter| parameter.name.as_str().to_owned())
                        .collect(),
                    capsule.published_buckets().to_vec(),
                    Some(program.state_signature_contracts(signature)),
                    !capsule.published_buckets().is_empty(),
                    capsule.target_contract_fingerprint(),
                )
            };
            let Some(call_site) = crate::find_call_site(
                program,
                state_flow.machine_symbol,
                state_flow.state_symbol,
                call_flow.statement_index,
                call_flow.call_ordinal,
            ) else {
                continue;
            };
            if matches!(call_site, crate::CallSite::TransitionNamed(_)) {
                // A named transition transfers within the current machine; it
                // is not an invocation of that machine's public crash ceiling.
                continue;
            }
            let route_expressions = uses_published_routes.then(|| {
                crash_route_expressions_by_identity(
                    program,
                    route_contracts.expect("published crash routes retain their contract set"),
                    &target_parameter_names,
                    content_conservation,
                )
            });
            let arguments = crate::call_site_argument_expressions(program, &call_site);
            let mut surviving_buckets = Vec::new();
            for bucket in &target_buckets {
                let mut surviving_guards = Vec::new();
                for guard in bucket.alternative_guards() {
                    match guard {
                        psi_checked_trees::CrashRouteGuard::Truth => {
                            surviving_guards.push(psi_checked_trees::CrashRouteGuard::Truth);
                        }
                        psi_checked_trees::CrashRouteGuard::Predicate(identity) => {
                            let expression = *route_expressions
                                .as_ref()
                                .and_then(|expressions| expressions.get(identity))
                                .expect(
                                "a canonical published crash route retains its typed producer expression",
                            );
                            match crate::checks::contracts::call_site_boolean_contract_expression_value(
                                program,
                                state_flow,
                                call_flow,
                                &call_site,
                                target_state_symbol,
                                target_parameters,
                                expression,
                            ) {
                                Some(false) => {}
                                Some(true) => surviving_guards
                                    .push(psi_checked_trees::CrashRouteGuard::Truth),
                                None => surviving_guards.push(
                                    psi_checked_trees::CrashRouteGuard::Predicate(
                                        canonical_instantiated_crash_route(
                                            program,
                                            expression,
                                            target_parameters,
                                            arguments,
                                            &caller_parameter_names,
                                            content_conservation,
                                        ),
                                    ),
                                ),
                            }
                        }
                    }
                }
                if let Some(bucket) = psi_checked_trees::CrashRouteBucket::new(
                    bucket.cause(),
                    bucket.containment_demand(),
                    surviving_guards,
                ) {
                    surviving_buckets.push(bucket);
                }
            }
            let caller_index = calls_by_caller
                .iter()
                .position(|(machine, _)| *machine == state_flow.machine_symbol)
                .unwrap_or_else(|| {
                    calls_by_caller.push((state_flow.machine_symbol, Vec::new()));
                    calls_by_caller.len() - 1
                });
            calls_by_caller[caller_index]
                .1
                .push(psi_checked_trees::CheckedCrashCallSite::new(
                    psi_checked_trees::CrashCallSiteLocation::new(
                        state_flow.state_symbol,
                        u32::try_from(call_flow.statement_index)
                            .expect("statement ordinal exceeds checked crash-call identity range"),
                        u32::try_from(call_flow.call_ordinal)
                            .expect("call ordinal exceeds checked crash-call identity range"),
                    ),
                    target_machine_symbol,
                    target_state_symbol,
                    target_contract_fingerprint,
                    surviving_buckets,
                ));
        }
    }

    for plan in plans {
        let checked_calls = calls_by_caller
            .iter_mut()
            .find(|(machine, _)| *machine == plan.machine)
            .map(|(_, calls)| std::mem::take(calls))
            .unwrap_or_default();
        plan.crash = plan
            .crash
            .clone()
            .with_checked_calls(checked_calls)
            .expect("one checked crash-call record occupies each invocation coordinate");
    }
}

fn infer_private_body_summaries(
    program: &TypedTrees,
    flow: &psi_checked_trees::FlowFacts,
    crash_capsules: &[psi_checked_trees::CrashContractCapsule],
    plans: &[psi_checked_trees::MachineContractPlan],
) -> Vec<(SymbolHandle, Vec<psi_checked_trees::CrashRouteBucket>)> {
    struct SummaryNode {
        machine: SymbolHandle,
        direct: Vec<psi_checked_trees::CrashRouteBucket>,
        invocations: Vec<(SymbolHandle, SymbolHandle)>,
    }

    let mut nodes = plans
        .iter()
        .filter(|target| {
            target.supply_mode == psi_language_semantics::MachineSupplyMode::CheckedBody
                && target.crash.published().is_empty()
        })
        .filter_map(|target| {
            Some(SummaryNode {
                machine: target.machine,
                direct: inferred_direct_body_crash_buckets(target),
                invocations: machine_non_transition_invocation_targets(
                    program,
                    flow,
                    target.machine,
                )?,
            })
        })
        .collect::<Vec<_>>();

    // Compute the greatest viable local subgraph. A recursive SCC is viable
    // when all of its outgoing targets are known local plans or pinned
    // requirement capsules. Any unresolved dependency removes its caller and
    // then every private caller that depended on it.
    loop {
        let viable_machines = nodes.iter().map(|node| node.machine).collect::<Vec<_>>();
        let before = nodes.len();
        nodes.retain(|node| {
            node.invocations.iter().all(|(machine, state)| {
                if let Some(plan) = plans.iter().find(|plan| plan.machine == *machine) {
                    plan.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody
                        || !plan.crash.published().is_empty()
                        || viable_machines.contains(machine)
                } else {
                    crash_capsules.iter().any(|capsule| {
                        capsule.target_machine() == *machine && capsule.target_state() == *state
                    })
                }
            })
        });
        if nodes.len() == before {
            break;
        }
    }

    let equations = nodes
        .iter()
        .map(|node| {
            let mut private_dependencies = Vec::new();
            let mut published_dependencies = Vec::new();
            for (invoked_machine, invoked_state) in &node.invocations {
                if let Some(plan) = plans.iter().find(|plan| plan.machine == *invoked_machine) {
                    if plan.supply_mode == psi_language_semantics::MachineSupplyMode::CheckedBody
                        && plan.crash.published().is_empty()
                    {
                        private_dependencies.push(*invoked_machine);
                    } else {
                        published_dependencies.extend_from_slice(plan.crash.published());
                    }
                } else {
                    let capsule = crash_capsules
                        .iter()
                        .find(|capsule| {
                            capsule.target_machine() == *invoked_machine
                                && capsule.target_state() == *invoked_state
                        })
                        .expect("the viability pass retained only pinned requirement targets");
                    published_dependencies.extend_from_slice(capsule.published_buckets());
                }
            }
            PrivateSummaryEquation {
                machine: node.machine,
                direct: node.direct.clone(),
                private_dependencies,
                published_dependencies,
            }
        })
        .collect::<Vec<_>>();
    solve_private_summary_fixed_point(&equations)
}

struct PrivateSummaryEquation {
    machine: SymbolHandle,
    direct: Vec<psi_checked_trees::CrashRouteBucket>,
    private_dependencies: Vec<SymbolHandle>,
    published_dependencies: Vec<psi_checked_trees::CrashRouteBucket>,
}

fn solve_private_summary_fixed_point(
    equations: &[PrivateSummaryEquation],
) -> Vec<(SymbolHandle, Vec<psi_checked_trees::CrashRouteBucket>)> {
    let mut resolved = equations
        .iter()
        .map(|equation| (equation.machine, equation.direct.clone()))
        .collect::<Vec<_>>();
    for _ in 0..=equations.len() {
        let mut changed = false;
        for equation in equations {
            let mut buckets = equation.direct.clone();
            let selected = equation.published_dependencies.iter().chain(
                equation.private_dependencies.iter().flat_map(|dependency| {
                    resolved
                        .iter()
                        .find(|(machine, _)| machine == dependency)
                        .expect("every private dependency belongs to the viable fixed point")
                        .1
                        .iter()
                }),
            );
            buckets.extend(selected.map(|bucket| {
                // Predicate producers may belong to a deeper body. Until
                // guarded substitution is retained, propagation keeps
                // cause/scope and fails safely to an unconditional route.
                psi_checked_trees::CrashRouteBucket::unconditional(
                    bucket.cause(),
                    bucket.containment_demand(),
                )
            }));
            buckets.sort();
            buckets.dedup();
            let (_, current) = resolved
                .iter_mut()
                .find(|(machine, _)| *machine == equation.machine)
                .expect("every viable node starts with a direct summary");
            if *current != buckets {
                *current = buckets;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    resolved
}

fn machine_non_transition_invocation_targets(
    program: &TypedTrees,
    flow: &psi_checked_trees::FlowFacts,
    machine: SymbolHandle,
) -> Option<Vec<(SymbolHandle, SymbolHandle)>> {
    let mut targets = Vec::new();
    for (_, state) in flow
        .control
        .states
        .iter()
        .filter(|(_, state)| state.machine_symbol == machine)
    {
        for call in flow.control.calls.span_or_empty(state.calls) {
            let site = crate::find_call_site(
                program,
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            )?;
            if matches!(site, crate::CallSite::TransitionNamed(_)) {
                continue;
            }
            let (target_machine, target_state) =
                crate::contract_target_from_state_symbol(program, call.target_symbol)?;
            if !targets.contains(&(target_machine, target_state)) {
                targets.push((target_machine, target_state));
            }
        }
    }
    Some(targets)
}

fn inferred_direct_body_crash_buckets(
    target: &psi_checked_trees::MachineContractPlan,
) -> Vec<psi_checked_trees::CrashRouteBucket> {
    let mut buckets = target
        .crash
        .checked_sites()
        .iter()
        .map(|site| {
            psi_checked_trees::CrashRouteBucket::unconditional(site.cause(), site.damage_minimum())
        })
        .collect::<Vec<_>>();
    buckets.sort();
    buckets.dedup();
    buckets
}

fn requirement_signature<'program>(
    program: &'program TypedTrees,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
) -> Option<&'program psi_typed_trees::signature::StateSignature> {
    if target_machine == target_state {
        return program
            .machine_parameter_signature(target_state)
            .map(|(_, signature)| signature);
    }
    program
        .traits()
        .iter()
        .find(|definition| definition.symbol == target_machine)
        .and_then(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .find(|signature| signature.symbol == target_state)
        })
}

fn crash_route_expressions_by_identity(
    program: &TypedTrees,
    contracts: &[psi_typed_trees::signature::SignatureContract],
    parameter_names: &[String],
    content_conservation: &[psi_validation::ContentConservationSourcePlan],
) -> std::collections::BTreeMap<
    psi_checked_trees::CrashPredicateIdentity,
    psi_typed_trees::expression::ExpressionHandle,
> {
    let mut expressions = std::collections::BTreeMap::new();
    for contract in contracts {
        if !matches!(
            contract.kind,
            psi_typed_trees::signature::SignatureContractKind::Crashes { .. }
        ) {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let psi_typed_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            if is_true_crash_route(program, fact) {
                continue;
            }
            let mut bytes = Vec::new();
            encode_contract_fact_canonical(
                program,
                fact,
                parameter_names,
                content_conservation,
                false,
                &mut bytes,
            );
            expressions.insert(
                psi_checked_trees::CrashPredicateIdentity::from_canonical_bytes(bytes),
                *expression,
            );
        }
    }
    expressions
}

/// Canonicalize a callee route after replacing bare formal parameters with
/// this invocation's argument expressions. The result lives in the caller's
/// positional namespace, so parameter renames on either side remain
/// irrelevant and checked consumers need no source handles.
fn canonical_instantiated_crash_route(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    target_parameters: &[psi_typed_trees::signature::StateParameter],
    arguments: &[psi_typed_trees::expression::ExpressionHandle],
    caller_parameter_names: &[String],
    content_conservation: &[psi_validation::ContentConservationSourcePlan],
) -> psi_checked_trees::CrashPredicateIdentity {
    let mut bytes = vec![1]; // ProofFact::Expression
    encode_instantiated_crash_expression(
        program,
        expression,
        target_parameters,
        arguments,
        caller_parameter_names,
        content_conservation,
        &mut bytes,
    );
    psi_checked_trees::CrashPredicateIdentity::from_canonical_bytes(bytes)
}

fn encode_instantiated_crash_expression(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    target_parameters: &[psi_typed_trees::signature::StateParameter],
    arguments: &[psi_typed_trees::expression::ExpressionHandle],
    caller_parameter_names: &[String],
    content_conservation: &[psi_validation::ContentConservationSourcePlan],
    out: &mut Vec<u8>,
) {
    use psi_typed_trees::expression::ExpressionNode;

    if let Some(conservation) = content_conservation
        .iter()
        .find(|candidate| candidate.source_expression == expression)
    {
        out.push(0xcc);
        out.extend(
            psi_language_semantics::content::content_conservation_plan_bytes(&conservation.plan),
        );
        return;
    }
    if !expression.is_valid() {
        out.push(0);
        return;
    }

    if let ExpressionNode::Name(path) = program.expression_table.expression(expression) {
        let name = program
            .expression_table
            .name_path_members(path.members)
            .last()
            .map(|name| name.as_str());
        let mut argument_index = 0usize;
        for parameter in target_parameters {
            let matches = (path.head_symbol.is_valid() && path.head_symbol == parameter.symbol)
                || (path.symbol.is_valid() && path.symbol == parameter.symbol)
                || name.is_some_and(|name| name == parameter.name.as_str());
            if parameter.is_self {
                if matches {
                    break;
                }
                continue;
            }
            let argument = arguments.get(argument_index).copied();
            argument_index = argument_index.saturating_add(1);
            if matches {
                if let Some(argument) = argument {
                    encode_expression_canonical(program, argument, caller_parameter_names, out);
                    return;
                }
                break;
            }
        }
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            out.push(1);
            out.push(binary.operator as u8);
            encode_instantiated_crash_expression(
                program,
                binary.left,
                target_parameters,
                arguments,
                caller_parameter_names,
                content_conservation,
                out,
            );
            encode_instantiated_crash_expression(
                program,
                binary.right,
                target_parameters,
                arguments,
                caller_parameter_names,
                content_conservation,
                out,
            );
        }
        ExpressionNode::Unary(unary) => {
            out.push(2);
            out.push(unary.operator as u8);
            encode_instantiated_crash_expression(
                program,
                unary.operand,
                target_parameters,
                arguments,
                caller_parameter_names,
                content_conservation,
                out,
            );
        }
        ExpressionNode::Integer(value) => {
            out.push(3);
            out.extend(value.text().as_bytes());
            out.push(0);
        }
        ExpressionNode::Boolean(value) => {
            out.push(4);
            out.push(u8::from(*value));
        }
        ExpressionNode::Name(path) => {
            out.push(5);
            for member in program.expression_table.name_path_members(path.members) {
                out.extend(member.as_str().as_bytes());
                out.push(b'.');
            }
            out.push(0);
        }
        ExpressionNode::Member(member) => {
            out.push(6);
            encode_instantiated_crash_expression(
                program,
                member.receiver,
                target_parameters,
                arguments,
                caller_parameter_names,
                content_conservation,
                out,
            );
            out.extend(member.member.as_str().as_bytes());
            out.push(0);
        }
        ExpressionNode::Call(call) => {
            out.push(7);
            out.extend(call.target.as_str().as_bytes());
            out.push(0);
            encode_instantiated_crash_expression(
                program,
                call.receiver,
                target_parameters,
                arguments,
                caller_parameter_names,
                content_conservation,
                out,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                encode_instantiated_crash_expression(
                    program,
                    *argument,
                    target_parameters,
                    arguments,
                    caller_parameter_names,
                    content_conservation,
                    out,
                );
            }
            out.push(0xfe);
        }
        other => {
            let _ = other;
            out.push(8);
            out.extend(program.expression_table.display_name(expression).as_bytes());
            out.push(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_summary_fixed_point_closes_recursive_components() {
        let first = SymbolHandle::from_arena_index(1);
        let second = SymbolHandle::from_arena_index(2);
        let abort = psi_checked_trees::CrashRouteBucket::unconditional(
            psi_checked_trees::CrashCause::Abort,
            psi_checked_trees::EXECUTION_DOMAIN_CRASH_SCOPE,
        );
        let trap = psi_checked_trees::CrashRouteBucket::unconditional(
            psi_checked_trees::CrashCause::Trap,
            psi_checked_trees::ACTIVATION_CRASH_SCOPE,
        );
        let equations = vec![
            PrivateSummaryEquation {
                machine: first,
                direct: Vec::new(),
                private_dependencies: vec![second],
                published_dependencies: vec![abort.clone()],
            },
            PrivateSummaryEquation {
                machine: second,
                direct: vec![trap.clone()],
                private_dependencies: vec![first],
                published_dependencies: Vec::new(),
            },
        ];

        let summaries = solve_private_summary_fixed_point(&equations);
        for machine in [first, second] {
            let buckets = summaries
                .iter()
                .find_map(|(candidate, buckets)| (*candidate == machine).then_some(buckets))
                .expect("each recursive member has a summary");
            assert!(buckets.contains(&abort));
            assert!(buckets.contains(&trap));
        }
    }
}
