//! Private body crash summaries and their fixed point.

use crate::facts::crash_calls::crash_predicate_from_expression;
use crate::facts::crash_calls::route_substitution::{
    call_argument_substitution, refine_published_crash_routes,
};
use crate::facts::crash_calls::summary_predicates::{
    CallArgumentSubstitution, SummaryCrashBucket, SummaryCrashPredicate, SummaryCrashRouteGuard,
    normalize_summary_buckets,
};
use crate::facts::crash_plan_facts::is_true_crash_route;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

pub(crate) fn infer_private_body_summaries(
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    semantic: &facts::FactPlan,
    flow: &checked_trees::FlowFacts,
    content_conservation: &[validation::ContentConservationSourcePlan],
    crash_capsules: &[checked_trees::CrashContractCapsule],
    plans: &[checked_trees::MachineContractPlan],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Vec<(SymbolHandle, Vec<SummaryCrashBucket>)> {
    let mut nodes = plans
        .iter()
        .filter(|target| {
            crate::lookup::machine_by_symbol(program, target.machine).is_some_and(|machine| {
                machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
            }) && target.crash.published().is_empty()
        })
        .filter_map(|target| {
            Some(SummaryNode {
                machine: target.machine,
                direct: inferred_direct_body_crash_buckets(target),
                invocations: machine_non_transition_invocation_sites(
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
            node.invocations.iter().all(|invocation| {
                if let Some(plan) = plans
                    .iter()
                    .find(|plan| plan.machine == invocation.target_machine)
                {
                    crate::lookup::machine_by_symbol(program, plan.machine).is_some_and(|machine| {
                        machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                            || !plan.crash.published().is_empty()
                            || viable_machines.contains(&invocation.target_machine)
                    })
                } else {
                    crash_capsules.iter().any(|capsule| {
                        capsule.target_machine() == invocation.target_machine
                            && capsule.target_state() == invocation.target_state
                    })
                }
            })
        });
        if nodes.len() == before {
            break;
        }
    }

    let mut equations = Vec::new();
    for node in &nodes {
        let mut private_dependencies = Vec::new();
        let mut published_dependencies = Vec::new();
        for invocation in &node.invocations {
            let state_flow = flow
                .control
                .states
                .iter()
                .find_map(|(_, state)| {
                    (state.machine_symbol == node.machine
                        && state.state_symbol == invocation.caller_state)
                        .then_some(state)
                })
                .expect("a retained summary invocation has its flow state");
            let call_flow = flow
                .control
                .calls
                .span_or_empty(state_flow.calls)
                .iter()
                .find(|call| {
                    call.statement_index == invocation.statement_index
                        && call.call_ordinal == invocation.call_ordinal
                })
                .expect("a retained summary invocation has its flow call");
            let call_site = crate::semantic_calls::find_call_site(
                program,
                node.machine,
                invocation.caller_state,
                invocation.statement_index,
                invocation.call_ordinal,
            )
            .expect("a retained summary invocation has its typed call site");
            let arguments =
                crate::semantic_calls::call_site_argument_expressions(program, &call_site);

            if let Some(target_plan) = plans
                .iter()
                .find(|plan| plan.machine == invocation.target_machine)
            {
                let target_machine =
                    crate::lookup::machine_by_symbol(program, invocation.target_machine)
                        .expect("a local crash plan has its typed machine");
                let target_state = program
                    .machine_states(target_machine)
                    .iter()
                    .find(|state| state.symbol == invocation.target_state)
                    .expect("a local crash invocation has its typed state");
                let target_parameters = program.state_parameters(target_state);
                let target_parameter_names = program
                    .machine_states(target_machine)
                    .first()
                    .map(|entry| {
                        program
                            .state_parameters(entry)
                            .iter()
                            .map(|parameter| parameter.name.as_str().to_owned())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if target_machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
                    && target_plan.crash.published().is_empty()
                {
                    private_dependencies.push(PrivateSummaryDependency {
                        machine: invocation.target_machine,
                        substitution: call_argument_substitution(
                            program,
                            operators,
                            semantic,
                            flow,
                            state_flow,
                            call_flow,
                            target_parameters,
                            arguments,
                            exact_integer_casts,
                        ),
                        recursive: private_dependency_reaches(
                            program,
                            &nodes,
                            plans,
                            invocation.target_machine,
                            node.machine,
                        ),
                    });
                } else if !target_plan.crash.published().is_empty() {
                    published_dependencies.extend(refine_published_crash_routes(
                        program,
                        operators,
                        exact_integer_casts,
                        semantic,
                        flow,
                        state_flow,
                        call_flow,
                        &call_site,
                        invocation.target_state,
                        target_parameters,
                        &target_parameter_names,
                        target_plan.crash.published(),
                        program.machine_contracts(target_machine),
                        content_conservation,
                        call_frames,
                    ));
                }
            } else {
                let capsule = crash_capsules
                    .iter()
                    .find(|capsule| {
                        capsule.target_machine() == invocation.target_machine
                            && capsule.target_state() == invocation.target_state
                    })
                    .expect("the viability pass retained only pinned requirement targets");
                if capsule.published_buckets().is_empty() {
                    continue;
                }
                let signature = requirement_signature(
                    program,
                    invocation.target_machine,
                    invocation.target_state,
                )
                .expect("a pinned requirement capsule has its typed signature");
                let target_parameters = program.state_signature_parameters(signature);
                let target_parameter_names = target_parameters
                    .iter()
                    .map(|parameter| parameter.name.as_str().to_owned())
                    .collect::<Vec<_>>();
                published_dependencies.extend(refine_published_crash_routes(
                    program,
                    operators,
                    exact_integer_casts,
                    semantic,
                    flow,
                    state_flow,
                    call_flow,
                    &call_site,
                    invocation.target_state,
                    target_parameters,
                    &target_parameter_names,
                    capsule.published_buckets(),
                    program.state_signature_contracts(signature),
                    content_conservation,
                    call_frames,
                ));
            }
        }
        equations.push(PrivateSummaryEquation {
            machine: node.machine,
            direct: node.direct.clone(),
            private_dependencies,
            published_dependencies: normalize_summary_buckets(published_dependencies),
        });
    }
    solve_private_summary_fixed_point(&equations)
}

struct SummaryNode {
    machine: SymbolHandle,
    direct: Vec<SummaryCrashBucket>,
    invocations: Vec<SummaryInvocationSite>,
}

#[derive(Debug, Clone, Copy)]
struct SummaryInvocationSite {
    caller_state: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
}

pub(crate) struct PrivateSummaryEquation {
    pub(crate) machine: SymbolHandle,
    pub(crate) direct: Vec<SummaryCrashBucket>,
    pub(crate) private_dependencies: Vec<PrivateSummaryDependency>,
    pub(crate) published_dependencies: Vec<SummaryCrashBucket>,
}

pub(crate) struct PrivateSummaryDependency {
    pub(crate) machine: SymbolHandle,
    pub(crate) substitution: CallArgumentSubstitution,
    pub(crate) recursive: bool,
}

pub(crate) fn solve_private_summary_fixed_point(
    equations: &[PrivateSummaryEquation],
) -> Vec<(SymbolHandle, Vec<SummaryCrashBucket>)> {
    let mut resolved = equations
        .iter()
        .map(|equation| (equation.machine, equation.direct.clone()))
        .collect::<Vec<_>>();
    for _ in 0..=equations.len() {
        let mut changed = false;
        for equation in equations {
            let mut buckets = equation.direct.clone();
            buckets.extend(equation.published_dependencies.clone());
            for dependency in &equation.private_dependencies {
                let selected = &resolved
                    .iter()
                    .find(|(machine, _)| *machine == dependency.machine)
                    .expect("every private dependency belongs to the viable fixed point")
                    .1;
                buckets.extend(selected.iter().map(|bucket| {
                    if dependency.recursive {
                        // Substitution around a recursive cycle can create an
                        // unbounded family such as p(n), p(n - 1), ... . The
                        // finite conservative lattice widens exactly those SCC
                        // edges to their cause bucket.
                        SummaryCrashBucket::unconditional(bucket.cause)
                    } else {
                        bucket.substitute(&dependency.substitution)
                    }
                }));
            }
            let buckets = normalize_summary_buckets(buckets);
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

fn machine_non_transition_invocation_sites(
    program: &TypedTrees,
    flow: &checked_trees::FlowFacts,
    machine: SymbolHandle,
) -> Option<Vec<SummaryInvocationSite>> {
    let mut sites = Vec::new();
    for (_, state) in flow
        .control
        .states
        .iter()
        .filter(|(_, state)| state.machine_symbol == machine)
    {
        for call in flow.control.calls.span_or_empty(state.calls) {
            let site = crate::semantic_calls::find_call_site(
                program,
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            )?;
            if matches!(
                site,
                crate::semantic_calls::CallSite::TransitionNamed { .. }
            ) {
                continue;
            }
            let (target_machine, target_state) =
                crate::proof::contract_target_from_state_symbol(program, call.target_symbol)?;
            sites.push(SummaryInvocationSite {
                caller_state: state.state_symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target_machine,
                target_state,
            });
        }
    }
    Some(sites)
}

fn inferred_direct_body_crash_buckets(
    target: &checked_trees::MachineContractPlan,
) -> Vec<SummaryCrashBucket> {
    let mut buckets = target
        .crash
        .checked_sites()
        .iter()
        .map(|site| SummaryCrashBucket::unconditional(site.cause()))
        .collect::<Vec<_>>();
    // Selected operators are direct invocations in this body's summary, not
    // fabricated machine-call edges. Their producer already distinguishes
    // captured operands from entry values and widens unknown origins to Truth.
    // Adding them here lets the existing private-call fixed point propagate
    // their causes, including through recursive wrappers.
    for operator in target.crash.checked_operators() {
        for bucket in &operator.surviving {
            buckets.push(SummaryCrashBucket {
                cause: bucket.cause(),
                alternative_guards: bucket
                    .alternative_guards()
                    .iter()
                    .map(|guard| match guard {
                        checked_trees::CrashRouteGuard::Truth => SummaryCrashRouteGuard::Truth,
                        checked_trees::CrashRouteGuard::Predicate(predicate) => predicate
                            .expression()
                            .map_or(SummaryCrashRouteGuard::Truth, |identity| {
                                SummaryCrashRouteGuard::Predicate(SummaryCrashPredicate {
                                    identity: identity.clone(),
                                    builtin_meaning: false,
                                    scalar: predicate.scalar_expression().cloned(),
                                })
                            }),
                    })
                    .collect(),
            });
        }
    }
    normalize_summary_buckets(buckets)
}

fn private_dependency_reaches(
    program: &TypedTrees,
    nodes: &[SummaryNode],
    plans: &[checked_trees::MachineContractPlan],
    start: SymbolHandle,
    goal: SymbolHandle,
) -> bool {
    let mut pending = vec![start];
    let mut visited = Vec::new();
    while let Some(machine) = pending.pop() {
        if machine == goal {
            return true;
        }
        if visited.contains(&machine) {
            continue;
        }
        visited.push(machine);
        let Some(node) = nodes.iter().find(|node| node.machine == machine) else {
            continue;
        };
        pending.extend(node.invocations.iter().filter_map(|invocation| {
            plans
                .iter()
                .find(|plan| plan.machine == invocation.target_machine)
                .filter(|plan| {
                    crate::lookup::machine_by_symbol(program, plan.machine).is_some_and(|machine| {
                        machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
                            && plan.crash.published().is_empty()
                    })
                })
                .map(|_| invocation.target_machine)
        }));
    }
    false
}

pub(crate) fn requirement_signature(
    program: &TypedTrees,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
) -> Option<&typed_trees::signature::StateSignature> {
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

pub(crate) fn crash_route_expressions_by_identity(
    program: &TypedTrees,
    contracts: &[typed_trees::signature::SignatureContract],
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> std::collections::BTreeMap<
    checked_trees::CrashPredicateIdentity,
    typed_trees::expression::ExpressionHandle,
> {
    let mut expressions = std::collections::BTreeMap::new();
    for contract in contracts {
        if !matches!(
            contract.kind,
            typed_trees::signature::SignatureContractKind::Crashes { .. }
        ) {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            if is_true_crash_route(program, fact) {
                continue;
            }
            let structured = crash_predicate_from_expression(
                program,
                *expression,
                parameter_names,
                Some(content_conservation),
            );
            expressions.insert(
                checked_trees::CrashPredicateIdentity::from_expression(structured),
                *expression,
            );
        }
    }
    expressions
}
