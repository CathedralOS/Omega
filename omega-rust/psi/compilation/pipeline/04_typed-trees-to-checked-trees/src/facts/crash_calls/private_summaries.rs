//! Private body and target-family crash summaries share one fixed point.
//! Family membership is a summary dependency, never an executable call site.

use crate::facts::crash_calls::crash_predicate_from_expression;
use crate::facts::crash_calls::route_substitution::{
    call_argument_substitution, refine_published_crash_routes,
};
use crate::facts::crash_calls::summary_predicates::{
    CallArgumentSubstitution, SummaryCrashBucket, SummaryCrashPredicate, SummaryCrashRouteGuard,
    normalize_summary_buckets,
};
use symbols::SymbolHandle;
use validation::contract_structure::is_true_crash_route;

pub(crate) struct PreparedCrashSummaryGraph {
    nodes: Vec<SummaryNode>,
    family_potential_causes: Vec<(SymbolHandle, Vec<checked_trees::CrashCause>)>,
}

impl PreparedCrashSummaryGraph {
    pub(crate) fn family_potential_crash_causes(
        &self,
    ) -> &[(SymbolHandle, Vec<checked_trees::CrashCause>)] {
        &self.family_potential_causes
    }
}

/// Prepare one viable dependency graph for public potential causes and later
/// invocation refinement. Public causes never consume call-site proof pruning.
pub(crate) fn prepare_crash_summary_graph(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    flow: &checked_trees::FlowFacts,
    crash_capsules: &[checked_trees::CrashContractCapsule],
    plans: &[checked_trees::MachineContractPlan],
) -> Result<PreparedCrashSummaryGraph, Vec<diagnostics::Diagnostic>> {
    let nodes = prepare_summary_nodes(program, operators, flow, crash_capsules, plans);
    let incomplete = program
        .machines()
        .iter()
        .filter(|machine| {
            machine.target_family_interface
                && !nodes.iter().any(|node| node.machine == machine.symbol)
        })
        .map(|machine| {
            let reason = match complete_family_members(program, machine, plans) {
                Err(reason) => reason,
                Ok(members) => unviable_member_dependency(
                    program,
                    operators,
                    flow,
                    crash_capsules,
                    plans,
                    &nodes,
                    &members,
                )
                .unwrap_or_else(|| {
                    "incomplete or unresolved member invocation dependencies".to_owned()
                }),
            };
            diagnostics::Diagnostic::error(format!(
                "cannot publish target-family crash envelope for `{}`: {reason}",
                machine.name,
            ))
        })
        .collect::<Vec<_>>();
    if !incomplete.is_empty() {
        return Err(incomplete);
    }
    let family_potential_causes =
        super::potential_causes::infer(program, &nodes, crash_capsules, plans);
    Ok(PreparedCrashSummaryGraph {
        nodes,
        family_potential_causes,
    })
}

/// Name the first member dependency that kept a family out of the viable
/// summary graph, so the refusal points at the authored call that caused it.
fn unviable_member_dependency(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    flow: &checked_trees::FlowFacts,
    crash_capsules: &[checked_trees::CrashContractCapsule],
    plans: &[checked_trees::MachineContractPlan],
    nodes: &[SummaryNode],
    members: &[SymbolHandle],
) -> Option<String> {
    let viable = |machine: SymbolHandle| nodes.iter().any(|node| node.machine == machine);
    members.iter().find_map(|member| {
        let plan = checked_trees::contract_plan_by_machine(plans, *member)?;
        let machine = program.machine_by_symbol(*member)?;
        if !needs_inferred_summary(machine, plan) || viable(*member) {
            return None;
        }
        let member_name = program.symbols.display_path(*member, "::");
        let sites = match machine_non_transition_invocation_sites(program, operators, flow, plan) {
            Ok(sites) => sites,
            Err(reason) => return Some(format!("retained member `{member_name}` {reason}")),
        };
        sites.iter().find_map(|site| {
            let target = program.symbols.display_path(site.target_machine, "::");
            let Some(target_plan) =
                checked_trees::contract_plan_by_machine(plans, site.target_machine)
            else {
                let pinned = crash_capsules.iter().any(|capsule| {
                    capsule.target_machine() == site.target_machine
                        && capsule.target_state() == site.target_state
                });
                return (!pinned).then(|| {
                    format!("retained member `{member_name}` calls `{target}` without a pinned crash capsule")
                });
            };
            program
                .machine_by_symbol(site.target_machine)
                .filter(|target_machine| {
                    needs_inferred_summary(target_machine, target_plan)
                        && !viable(site.target_machine)
                })
                .map(|_| {
                    format!("retained member `{member_name}` calls `{target}`, whose crash summary is unresolved")
                })
        })
    })
}

fn prepare_summary_nodes(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    flow: &checked_trees::FlowFacts,
    crash_capsules: &[checked_trees::CrashContractCapsule],
    plans: &[checked_trees::MachineContractPlan],
) -> Vec<SummaryNode> {
    let mut nodes = plans
        .iter()
        .filter(|target| {
            program
                .machine_by_symbol(target.machine)
                .is_some_and(|machine| needs_inferred_summary(machine, target))
        })
        .filter_map(|target| {
            let machine = program.machine_by_symbol(target.machine)?;
            let family_members = if machine.target_family_interface {
                complete_family_members(program, machine, plans).ok()?
            } else {
                Vec::new()
            };
            Some(SummaryNode {
                machine: target.machine,
                direct: if machine.target_family_interface {
                    summary_published_crash_buckets(target.crash.published())
                } else {
                    inferred_direct_body_crash_buckets(target)
                },
                family_members,
                invocations: machine_non_transition_invocation_sites(
                    program, operators, flow, target,
                )
                .ok()?,
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
            node.family_members.iter().all(|member| {
                checked_trees::contract_plan_by_machine(plans, *member).is_some_and(|plan| {
                    program.machine_by_symbol(*member).is_some_and(|machine| {
                        !needs_inferred_summary(machine, plan) || viable_machines.contains(member)
                    })
                })
            }) && node.invocations.iter().all(|invocation| {
                if let Some(plan) =
                    checked_trees::contract_plan_by_machine(plans, invocation.target_machine)
                {
                    program
                        .machine_by_symbol(plan.machine)
                        .is_some_and(|machine| {
                            !needs_inferred_summary(machine, plan)
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
    nodes
}

pub(crate) fn infer_private_body_summaries(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    semantic: &facts::FactPlan,
    flow: &checked_trees::FlowFacts,
    content_conservation: &[validation::ContentConservationSourcePlan],
    crash_capsules: &[checked_trees::CrashContractCapsule],
    plans: &[checked_trees::MachineContractPlan],
    prepared: Option<&PreparedCrashSummaryGraph>,
) -> Vec<(SymbolHandle, Vec<SummaryCrashBucket>)> {
    let owned_nodes;
    let nodes = if let Some(graph) = prepared {
        &graph.nodes
    } else {
        owned_nodes = prepare_summary_nodes(program, operators, flow, crash_capsules, plans);
        &owned_nodes
    };

    let mut equations = Vec::new();
    for node in nodes {
        let mut private_dependencies = Vec::new();
        let mut published_dependencies = Vec::new();
        for member in &node.family_members {
            let target_plan = checked_trees::contract_plan_by_machine(plans, *member)
                .expect("a viable family member has its local contract plan");
            let target_machine = program
                .machine_by_symbol(*member)
                .expect("a viable family member has its typed declaration");
            if needs_inferred_summary(target_machine, target_plan) {
                private_dependencies.push(PrivateSummaryDependency {
                    machine: *member,
                    // All guards already use entry-parameter positions. Family
                    // signature admission pins the positional correspondence;
                    // no invocation actual or source-name substitution exists.
                    substitution: None,
                    recursive: false,
                });
            } else {
                published_dependencies.extend(summary_published_crash_buckets(
                    target_plan.crash.published(),
                ));
            }
        }
        for invocation in &node.invocations {
            let state_flow = flow
                .control
                .state(node.machine, invocation.caller_state)
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
            let call_site = validation::find_call_site(
                program,
                node.machine,
                invocation.caller_state,
                invocation.statement_index,
                invocation.call_ordinal,
            )
            .expect("a retained summary invocation has its typed call site");
            let arguments = validation::call_site_argument_expressions(program, &call_site);

            if let Some(target_plan) =
                checked_trees::contract_plan_by_machine(plans, invocation.target_machine)
            {
                let target_machine = program
                    .machine_by_symbol(invocation.target_machine)
                    .expect("a local crash plan has its typed machine");
                let target_state = program
                    .state_in_machine(target_machine, invocation.target_state)
                    .expect("a local crash invocation has its typed state");
                let target_parameters = program.state_parameters(target_state);
                let target_parameter_names =
                    crate::facts::machine_entry_parameter_names(program, target_machine);
                if needs_inferred_summary(target_machine, target_plan) {
                    private_dependencies.push(PrivateSummaryDependency {
                        machine: invocation.target_machine,
                        substitution: Some(call_argument_substitution(
                            program,
                            operators,
                            semantic,
                            flow,
                            state_flow,
                            call_flow,
                            target_parameters,
                            arguments,
                            exact_integer_casts,
                        )),
                        recursive: private_dependency_reaches(
                            program,
                            nodes,
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
                ));
            }
        }
        equations.push(PrivateSummaryEquation {
            machine: node.machine,
            direct: if program
                .machine_by_symbol(node.machine)
                .is_some_and(|machine| machine.target_family_interface)
            {
                // Publication precedes call refinement. Its conservative
                // potential causes cannot disappear through proof pruning.
                checked_trees::contract_plan_by_machine(plans, node.machine)
                    .map(|plan| summary_published_crash_buckets(plan.crash.published()))
                    .unwrap_or_else(|| node.direct.clone())
            } else {
                node.direct.clone()
            },
            private_dependencies,
            published_dependencies: normalize_summary_buckets(published_dependencies),
        });
    }
    solve_private_summary_fixed_point(&equations)
}

pub(super) struct SummaryNode {
    pub(super) machine: SymbolHandle,
    direct: Vec<SummaryCrashBucket>,
    pub(super) family_members: Vec<SymbolHandle>,
    pub(super) invocations: Vec<SummaryInvocationSite>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SummaryInvocationSite {
    caller_state: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
    pub(super) target_machine: SymbolHandle,
    pub(super) target_state: SymbolHandle,
}

pub(crate) struct PrivateSummaryEquation {
    pub(crate) machine: SymbolHandle,
    pub(crate) direct: Vec<SummaryCrashBucket>,
    pub(crate) private_dependencies: Vec<PrivateSummaryDependency>,
    pub(crate) published_dependencies: Vec<SummaryCrashBucket>,
}

pub(crate) struct PrivateSummaryDependency {
    pub(crate) machine: SymbolHandle,
    /// None is the positional identity of a family membership dependency.
    pub(crate) substitution: Option<CallArgumentSubstitution>,
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
                    } else if let Some(substitution) = &dependency.substitution {
                        bucket.substitute(substitution, None)
                    } else {
                        bucket.clone()
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

/// Every machine edge of one body, or why one call cannot become an edge.
fn machine_non_transition_invocation_sites(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    flow: &checked_trees::FlowFacts,
    plan: &checked_trees::MachineContractPlan,
) -> Result<Vec<SummaryInvocationSite>, String> {
    let mut sites = Vec::new();
    for state in flow.control.machine_states(plan.machine) {
        for call in flow.control.calls.span_or_empty(state.calls) {
            let site = validation::find_call_site(
                program,
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            )
            .ok_or_else(|| {
                format!(
                    "has no typed call site for statement {} call {}",
                    call.statement_index, call.call_ordinal
                )
            })?;
            if matches!(site, validation::CallSite::TransitionNamed { .. }) {
                continue;
            }
            // A checked register transfer is a compiler-owned instruction,
            // not an unresolved machine invocation. Its architectural fault
            // capability is distinct from an authored language crash route.
            // Operand calls retain separate flow rows and still join below.
            if program
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
                .and_then(|builtin| {
                    language_core::inline_assembly::AsmRegisterOperation::from_intrinsic_name(
                        builtin.name(),
                    )
                })
                .is_some_and(|operation| operation.is_admitted())
            {
                continue;
            }
            // Reserved byte observations are total primitive operations, not
            // machine dependencies. Their operand calls still have independent
            // flow rows and must retain their own crash summaries.
            if let validation::CallSite::Expression { expression, .. } = site
                && typed_trees::byte_predicates::byte_predicate_call(
                    program.typed_program(),
                    expression,
                )
                .is_some()
            {
                continue;
            }
            // Declaration operations retain an exact intrinsic selection,
            // not a machine/state target. Their declaration operands do not
            // create product execution edges; separately inventoried operand
            // calls still participate in this same crash dependency graph.
            if is_selected_build_operation(program, site) {
                continue;
            }
            // Named operators have flow call rows but are not machine edges.
            // Their failure routes are already included by direct-body buckets.
            // Join the exact checked occurrence, and require that crash-bearing
            // declarations retained their site before excluding this call row.
            if let validation::CallSite::Expression { expression, .. } = site
                && let Some((named_handle, named)) =
                    operators.named_uses.iter().find(|(_, named)| {
                        named.expression == expression
                            && named.selected_operator_symbol == call.target_symbol
                            && matches!(named.origin,
                            checked_trees::CheckedValueOrigin::StateStatement {
                                machine_symbol, state_symbol, statement_index, ..
                            } if machine_symbol == plan.machine
                                && state_symbol == state.state_symbol
                                && statement_index == call.statement_index)
                    })
            {
                let operator = typed_trees::operator::declaration_by_symbol(
                    program,
                    named.selected_operator_symbol,
                )
                .ok_or_else(|| "selects an operator with no typed declaration".to_owned())?;
                let declares_crashes =
                    program.operator_contracts(operator).iter().any(|contract| {
                        matches!(
                            contract.kind,
                            typed_trees::signature::SignatureContractKind::Crashes { .. }
                        )
                    });
                if declares_crashes
                    && !plan.crash.checked_operators().iter().any(|site| {
                        site.named_use == named_handle
                            && site.selected_operator == named.selected_operator_symbol
                    })
                {
                    return Err(
                        "has a crash-bearing operator use without a checked site".to_owned()
                    );
                }
                continue;
            }
            let (target_machine, target_state) =
                selected_invocation_target(program, site, call.target_symbol).ok_or_else(|| {
                    format!(
                        "calls `{}`, which selects no declaration",
                        spelled_callee(program, site)
                    )
                })?;
            sites.push(SummaryInvocationSite {
                caller_state: state.state_symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target_machine,
                target_state,
            });
        }
    }
    Ok(sites)
}

fn spelled_callee(
    program: &dyn ::validation::CheckingInput,
    site: validation::CallSite<'_>,
) -> String {
    match site {
        validation::CallSite::Statement(call) => call.target.as_str().to_owned(),
        validation::CallSite::Expression { call, .. } => call.target.as_str().to_owned(),
        validation::CallSite::TransitionNamed { path, .. } => {
            program.symbols.display_path(path.symbol, "::")
        }
    }
}

/// Read the retained call selection rather than recognizing a provider-default
/// body or guessing an intrinsic from a diagnostic spelling. Unknown ordinary
/// targets remain unresolved dependencies and invalidate their summary.
fn is_selected_build_operation(
    program: &dyn ::validation::CheckingInput,
    site: validation::CallSite<'_>,
) -> bool {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget, BuildOperation,
    };

    let selected = match site {
        validation::CallSite::Statement(call) if !call.target_symbol.is_valid() => call
            .authored_call_selection
            .and_then(|occurrence| program.authored_declaration_selections().get(occurrence))
            .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call)
            .map(|selection| selection.target()),
        validation::CallSite::Expression { expression, call } if !call.target_symbol.is_valid() => {
            let mut selections = program
                .expression_table
                .authored_selection_occurrences(expression)
                .filter_map(|occurrence| program.authored_declaration_selections().get(occurrence))
                .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call);
            let selected = selections.next().map(|selection| selection.target());
            if selections.next().is_some() {
                return false;
            }
            selected
        }
        _ => None,
    };
    let Some(AuthoredDeclarationSelectionTarget::Intrinsic(intrinsic)) = selected else {
        return false;
    };
    BuildOperation::ALL
        .iter()
        .any(|operation| operation.intrinsic() == intrinsic)
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
    // A Trapping primitive is its own unconditional `Trap` site, exactly like
    // an explicit crash: its trap predicate reads body values, never only
    // entry parameters, so no narrower guard is available to publish.
    if !target.crash.trapping_sites().is_empty() {
        buckets.push(SummaryCrashBucket::unconditional(
            checked_trees::CrashCause::Trap,
        ));
    }
    // Selected operators are direct invocations in this body's summary, not
    // fabricated machine-call edges. Their producer already distinguishes
    // captured operands from entry values and widens unknown origins to Truth.
    // Adding them here lets the existing private-call fixed point propagate
    // their causes, including through recursive wrappers.
    for operator in target.crash.checked_operators() {
        buckets.extend(summary_published_crash_buckets(&operator.surviving));
    }
    normalize_summary_buckets(buckets)
}

fn private_dependency_reaches(
    program: &dyn ::validation::CheckingInput,
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
        pending.extend(
            node.family_members
                .iter()
                .filter(|member| {
                    checked_trees::contract_plan_by_machine(plans, **member).is_some_and(|plan| {
                        program
                            .machine_by_symbol(**member)
                            .is_some_and(|machine| needs_inferred_summary(machine, plan))
                    })
                })
                .copied(),
        );
        pending.extend(node.invocations.iter().filter_map(|invocation| {
            checked_trees::contract_plan_by_machine(plans, invocation.target_machine)
                .filter(|plan| {
                    program
                        .machine_by_symbol(plan.machine)
                        .is_some_and(|machine| needs_inferred_summary(machine, plan))
                })
                .map(|_| invocation.target_machine)
        }));
    }
    false
}

pub(super) fn needs_inferred_summary(
    machine: &typed_trees::machine::Machine,
    plan: &checked_trees::MachineContractPlan,
) -> bool {
    machine.target_family_interface
        || (machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
            && plan.crash.published().is_empty())
}

/// Missing or unfinished members invalidate the summary rather than supplying
/// an empty crash envelope. The family itself has no executable body.
fn complete_family_members(
    program: &dyn ::validation::CheckingInput,
    interface: &typed_trees::machine::Machine,
    plans: &[checked_trees::MachineContractPlan],
) -> Result<Vec<SymbolHandle>, String> {
    let mut families = program
        .target_machine_families
        .iter()
        .filter(|family| family.canonical == interface.symbol);
    let family = families
        .next()
        .ok_or_else(|| "generated interface has no retained family roster".to_owned())?;
    if families.next().is_some() {
        return Err("generated interface has multiple retained family rosters".to_owned());
    }
    if family.members.is_empty() {
        return Err("retained family roster has no members".to_owned());
    }
    let parameters = program
        .machine_states(interface)
        .first()
        .map(|entry| program.state_parameters(entry))
        .ok_or_else(|| "generated interface has no entry telescope".to_owned())?;
    let mut members = Vec::with_capacity(family.members.len());
    for member in &family.members {
        let member_name = program.symbols.display_path(member.machine, "::");
        let machine = program
            .machine_by_symbol(member.machine)
            .ok_or_else(|| format!("retained member `{member_name}` has no typed declaration"))?;
        let entry = program
            .machine_states(machine)
            .first()
            .ok_or_else(|| format!("retained member `{member_name}` has no entry telescope"))?;
        let mut member_plans = plans.iter().filter(|plan| plan.machine == member.machine);
        member_plans
            .next()
            .ok_or_else(|| format!("retained member `{member_name}` has no owned crash plan"))?;
        if member_plans.next().is_some() {
            return Err(format!(
                "retained member `{member_name}` has multiple owned crash plans"
            ));
        }
        let failure = if machine.target_family_interface {
            Some("is itself a generated interface")
        } else if machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
            && !machine.body_is_present
        {
            Some("claims checked supply without a present body")
        } else if !machine.supplies_checked_body()
            && !program.machine_publishes_crash_ceiling(machine)
        {
            Some("has opaque supply without a published crash ceiling")
        } else if machine.body_status != typed_trees::machine::MachineBodyStatus::Complete {
            Some("has an incomplete declaration/body status")
        } else if machine.structural_type_equations_pending {
            Some("has pending structural type equations")
        } else if program.state_parameters(entry).len() != parameters.len() {
            Some("has a different entry parameter count")
        } else if members.contains(&member.machine) {
            Some("occurs twice in the retained roster")
        } else {
            None
        };
        if let Some(failure) = failure {
            return Err(format!("retained member `{member_name}` {failure}"));
        }
        members.push(member.machine);
    }
    Ok(members)
}

fn summary_published_crash_buckets(
    buckets: &[checked_trees::CrashRouteBucket],
) -> Vec<SummaryCrashBucket> {
    normalize_summary_buckets(
        buckets
            .iter()
            .map(|bucket| SummaryCrashBucket {
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
            })
            .collect(),
    )
}

pub(crate) fn requirement_signature(
    program: &dyn ::validation::CheckingInput,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
) -> Option<&typed_trees::signature::StateSignature> {
    if target_machine == target_state {
        return program
            .machine_parameter_signature(target_state)
            .map(|(_, signature)| signature);
    }
    program
        .trait_definition_by_symbol(target_machine)
        .and_then(|definition| program.trait_machine_signature_by_symbol(definition, target_state))
}

pub(crate) fn crash_route_expressions_by_identity(
    program: &dyn ::validation::CheckingInput,
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
            let Some(expression) = fact.as_expression() else {
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

/// A sealed quotient request computes by its explicitly selected ordinary
/// representative. Keep that dependency in crash inference before runtime
/// admission: an unknown or crashing representative must never disappear.
pub(crate) fn selected_invocation_target(
    program: &dyn ::validation::CheckingInput,
    site: validation::CallSite<'_>,
    ordinary_target: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    let selected = match site {
        validation::CallSite::Expression { call, .. } => call
            .quotient_operation
            .as_ref()
            .map(|request| request.representative_operation.symbol),
        _ => None,
    };
    crate::proof::contract_target_from_state_symbol(program, selected.unwrap_or(ordinary_target))
}
