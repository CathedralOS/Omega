//! Check which published crash conditions remain possible at each call.
//! A selected requirement keeps its full public signature and the argument
//! values evaluated for that invocation. Private helpers compose those same
//! conditions with their own checked bodies.
//! Symbolic conditions retain caller entry-value identities. Checked scalar
//! expressions also use captured values and bounds; missing evidence leaves
//! the possible crash in place.
//!
//! This file attaches checked crash calls and infers crash causes.
//! `summary_predicates.rs` combines and groups possible crash conditions.
//! `route_substitution.rs` replaces declared parameters with call arguments.
//! `private_summaries.rs` finds crashes that can propagate through private calls.
//! `requirement_contracts.rs` prepares each call's selected public signature.

mod potential_causes;
mod private_summaries;
mod requirement_contracts;
mod route_substitution;
mod summary_predicates;

mod entry_equations;
mod entry_paths;
mod float_observations;

use crate::flow::expressions;
use crate::published::facts::crash_calls::private_summaries::{
    infer_private_body_summaries, requirement_signature,
};
use crate::published::facts::crash_calls::route_substitution::{
    SelectedTargetCrashRoutes, call_argument_substitution, refine_published_crash_routes,
};
use crate::published::facts::crash_calls::summary_predicates::{
    SummaryCrashBucket, normalize_summary_buckets,
};
use crate::published::facts::crash_plan_facts::declared_parameter_names;
use crate::shared::call_formals::arguments_aligned_to_formals;
use checked_trees::CrashPredicateExpression;
pub(crate) use private_summaries::{
    PreparedCrashSummaryGraph, prepare_crash_summary_graph, selected_invocation_target,
};
use symbols::SymbolHandle;

pub(super) fn crash_predicate_from_expression(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    parameter_names: &[String],
    content_conservation: Option<&[validation::ContentConservationSourcePlan]>,
) -> CrashPredicateExpression {
    predicate_expression(program, expression, parameter_names, content_conservation)
}

fn predicate_expression(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
    parameter_names: &[String],
    content_conservation: Option<&[validation::ContentConservationSourcePlan]>,
) -> CrashPredicateExpression {
    use typed_trees::expression::ExpressionNode;

    if let Some(conservation) = content_conservation.and_then(|plans| {
        plans
            .iter()
            .find(|candidate| candidate.source_expression == expression)
    }) {
        return CrashPredicateExpression::ContentConservation(
            language_semantics::content::content_conservation_plan_bytes(&conservation.plan),
        );
    }
    if !expression.is_valid() {
        return CrashPredicateExpression::Invalid;
    }
    let lower =
        |operand| predicate_expression(program, operand, parameter_names, content_conservation);
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => CrashPredicateExpression::Binary {
            operator: binary.operator as u8,
            left: Box::new(lower(binary.left)),
            right: Box::new(lower(binary.right)),
        },
        ExpressionNode::Unary(unary) => CrashPredicateExpression::Unary {
            operator: unary.operator as u8,
            operand: Box::new(lower(unary.operand)),
        },
        ExpressionNode::Integer(value) => CrashPredicateExpression::Integer(value.to_string()),
        // A float literal is a closed leaf like `Integer`: its suffix-free
        // spelling is the whole identity, so the predicate keeps it
        // structured instead of flattening to `Opaque`.
        ExpressionNode::Float(value) => CrashPredicateExpression::Float(value.text().to_owned()),
        ExpressionNode::Boolean(value) => CrashPredicateExpression::Boolean(*value),
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if let [single] = members
                && let Some(index) = parameter_names
                    .iter()
                    .position(|name| name == single.as_str())
            {
                return CrashPredicateExpression::Parameter(
                    u32::try_from(index).expect("parameter index fits u32"),
                );
            }
            CrashPredicateExpression::Name(
                members
                    .iter()
                    .map(|member| member.as_str().to_owned())
                    .collect(),
            )
        }
        ExpressionNode::Member(member) => CrashPredicateExpression::Member {
            receiver: Box::new(lower(member.receiver)),
            member: member_entry_name(
                program,
                member,
                validation::effective_member_symbol(program, member.receiver, member),
            ),
        },
        // A `collection[index]` read keeps both children structured: either
        // may carry a formal, and substitution must reach each rather than
        // hide it inside a flattened `Opaque` display.
        ExpressionNode::Indexed(indexed) => CrashPredicateExpression::Indexed {
            collection: Box::new(lower(indexed.collection)),
            index: Box::new(lower(indexed.index)),
        },
        // A `start..end` operand keeps both bounds structured for the same
        // reason: either bound may carry a formal, and substitution must
        // reach it. The inclusivity flag is part of the identity.
        ExpressionNode::Range(range) => CrashPredicateExpression::Range {
            start: Box::new(lower(range.start)),
            end: Box::new(lower(range.end)),
            end_inclusive: range.end_inclusive,
        },
        ExpressionNode::Call(call) => CrashPredicateExpression::Call {
            target: call.target.as_str().to_owned(),
            receiver: Box::new(lower(call.receiver)),
            arguments: expressions(program)
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| lower(*argument))
                .collect(),
        },
        other => {
            let _ = other;
            CrashPredicateExpression::Opaque(program.expression_table.display_name(expression))
        }
    }
}

/// One completed private fixed point, shared by direct invocation attachment and
/// universal callable refinement. Absence is unresolved; a present empty row is
/// a proved negative guarantee. These implementation rows never enter authored
/// contract fingerprints.
pub(crate) struct CompletedBodyCrashSummaries {
    rows: Vec<(SymbolHandle, Vec<SummaryCrashBucket>)>,
}

impl CompletedBodyCrashSummaries {
    pub(crate) fn checked_buckets(
        &self,
        machine: SymbolHandle,
    ) -> Option<Vec<checked_trees::CrashRouteBucket>> {
        let mut rows = self.rows.iter().filter(|(owner, _)| *owner == machine);
        let (_, buckets) = rows.next()?;
        if rows.next().is_some() {
            return None;
        }
        Some(
            buckets
                .iter()
                .cloned()
                .filter_map(SummaryCrashBucket::into_checked)
                .collect(),
        )
    }
}

/// Materialize direct invocation-specific crash refinement while the typed
/// expressions are still available. Selection uses a published ceiling when
/// one exists and a conservative monotone checked-body summary for same-unit
/// private machines. Recursive components close over their finite cause/scope
/// buckets; a dependency outside the recognized local/capsule graph keeps its
/// caller unexamined. The retained rows are entirely checked data: downstream
/// propagation can distinguish a proved-crash-free call from an unexamined call
/// without reopening source trees.
pub(super) fn attach_checked_crash_calls(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    semantic: &facts::FactPlan,
    flow: &checked_trees::FlowFacts,
    content_conservation: &[validation::ContentConservationSourcePlan],
    crash_capsules: &[checked_trees::CrashContractCapsule],
    plans: &mut [checked_trees::MachineContractPlan],
    summary_graph: &PreparedCrashSummaryGraph,
) -> CompletedBodyCrashSummaries {
    let inferred_body_summaries = CompletedBodyCrashSummaries {
        rows: infer_private_body_summaries(
            program,
            operators,
            exact_integer_casts,
            semantic,
            flow,
            content_conservation,
            crash_capsules,
            plans,
            summary_graph,
        ),
    };
    let mut plan_indices = symbols::SymbolKeyMap::<SymbolHandle, usize>::default();
    let mut plan_entry_parameter_names = Vec::with_capacity(plans.len());
    for (index, plan) in plans.iter().enumerate() {
        plan_indices.entry(plan.machine).or_insert(index);
        let entry_names = program
            .machine_by_symbol(plan.machine)
            .and_then(|machine| program.machine_states(machine).first())
            .map(|entry| declared_parameter_names(program.state_parameters(entry)))
            .unwrap_or_default();
        plan_entry_parameter_names.push(entry_names);
    }
    let mut capsule_by_target = symbols::SymbolKeyMap::<
        (SymbolHandle, SymbolHandle),
        &checked_trees::CrashContractCapsule,
    >::default();
    for capsule in crash_capsules {
        capsule_by_target
            .entry((capsule.target_machine(), capsule.target_state()))
            .or_insert(capsule);
    }
    let mut summary_by_machine =
        symbols::SymbolKeyMap::<SymbolHandle, &[SummaryCrashBucket]>::default();
    for (machine, summary) in &inferred_body_summaries.rows {
        summary_by_machine
            .entry(*machine)
            .or_insert(summary.as_slice());
    }
    let state_flows: Vec<&checked_trees::FlowStateFact> = flow
        .control
        .states
        .iter()
        .map(|(_, state_flow)| state_flow)
        .collect();
    let per_state_calls: Vec<Vec<(SymbolHandle, checked_trees::CheckedCrashCallSite)>> =
        validation::checking_thread_pool().install(|| {
            use rayon::prelude::*;
            state_flows
                .par_iter()
                .map(|&state_flow| {
                    let mut state_calls = Vec::new();
                    for call_flow in flow.control.calls.span_or_empty(state_flow.calls) {
                        let Some(call_site) = validation::find_call_site(
                            program,
                            state_flow.machine_symbol,
                            state_flow.state_symbol,
                            call_flow.statement_index,
                            call_flow.call_ordinal,
                        ) else {
                            continue;
                        };
                        let public_target = summary_graph
                            .requirement_contracts
                            .for_call(
                                state_flow.machine_symbol,
                                state_flow.state_symbol,
                                call_flow.statement_index,
                                call_flow.call_ordinal,
                            )
                            .map(|selected| {
                                (selected.capsule.target_machine(), selected.capsule.target_state())
                            });
                        let Some((target_machine_symbol, target_state_symbol)) = public_target
                            .or_else(|| {
                                private_summaries::selected_invocation_target(
                                    program,
                                    call_site,
                                    call_flow.target_symbol,
                                )
                            })
                        else {
                            continue;
                        };
                        let local_target = program.machine_by_symbol(target_machine_symbol);
                        let local_plan_index = plan_indices.get(&target_machine_symbol).copied();
                        let local_plan = local_plan_index.and_then(|index| plans.get(index));
                        let (
                            target_parameters,
                            target_parameter_names_owned,
                            target_parameter_names_ref,
                            target_routes,
                            target_contract_fingerprint,
                            target_contract_commitment,
                        ) = if let (Some(target_machine), Some(target_plan), Some(plan_index)) =
                            (local_target, local_plan, local_plan_index)
                        {
                            let Some(target_state) =
                                program.state_in_machine(target_machine, target_state_symbol)
                            else {
                                continue;
                            };
                            let target_routes = if target_machine.target_family_interface {
                                let Some(&summary) = summary_by_machine.get(&target_machine_symbol)
                                else {
                                    // Authored clauses alone do not close a family's private
                                    // inferred body causes or unresolved member dependencies.
                                    continue;
                                };
                                SelectedTargetCrashRoutes::Private(summary)
                            } else if !target_plan.crash.published().is_empty() {
                                SelectedTargetCrashRoutes::Published {
                                    buckets: target_plan.crash.published(),
                                    contracts: program.machine_contracts(target_machine),
                                }
                            } else if target_machine.supply_mode
                                == language_semantics::MachineSupplyMode::CheckedBody
                            {
                                let Some(&summary) = summary_by_machine.get(&target_machine_symbol)
                                else {
                                    // Dependency-unresolved private bodies remain
                                    // unexamined rather than erasing a nested crash.
                                    continue;
                                };
                                SelectedTargetCrashRoutes::Private(summary)
                            } else {
                                // Omission on a requirement/boundary/exported interface is
                                // the published negative guarantee, so retain an empty row
                                // as positive crash-free evidence.
                                SelectedTargetCrashRoutes::Empty
                            };
                            (
                                program.state_parameters(target_state),
                                Vec::new(),
                                Some(plan_entry_parameter_names[plan_index].as_slice()),
                                target_routes,
                                target_plan.report_fingerprint,
                                target_plan.commitment,
                            )
                        } else {
                            let selected = summary_graph.requirement_contracts.for_call(
                                state_flow.machine_symbol,
                                state_flow.state_symbol,
                                call_flow.statement_index,
                                call_flow.call_ordinal,
                            );
                            let (capsule, signature) = if let Some(selected) = selected {
                                (&selected.capsule, &selected.source.signature)
                            } else {
                                let Some(&capsule) = capsule_by_target
                                    .get(&(target_machine_symbol, target_state_symbol))
                                else {
                                    continue;
                                };
                                let Some(signature) = requirement_signature(
                                    program,
                                    target_machine_symbol,
                                    target_state_symbol,
                                ) else {
                                    continue;
                                };
                                (capsule, signature)
                            };
                            let parameters = program.state_signature_parameters(signature);
                            (
                                parameters,
                                declared_parameter_names(parameters),
                                None,
                                if capsule.published_buckets().is_empty() {
                                    SelectedTargetCrashRoutes::Empty
                                } else {
                                    SelectedTargetCrashRoutes::Published {
                                        buckets: capsule.published_buckets(),
                                        contracts: program.state_signature_contracts(signature),
                                    }
                                },
                                capsule.target_contract_report_fingerprint(),
                                capsule.target_contract_commitment(),
                            )
                        };
                        if call_site.is_internal_state_transfer(program, state_flow.machine_symbol)
                        {
                            // Only local state arrivals stay inside this invocation.
                            // Terminal machine calls retain ordinary crash obligations.
                            continue;
                        }
                        let target_parameter_names = target_parameter_names_ref
                            .unwrap_or(target_parameter_names_owned.as_slice());
                        let arguments = arguments_aligned_to_formals(
                            program,
                            call_flow.target_symbol,
                            target_parameters.len(),
                            validation::call_site_argument_expressions(program, &call_site),
                        );
                        let mut surviving_summary = match target_routes {
                            SelectedTargetCrashRoutes::Published { buckets, contracts } => {
                                refine_published_crash_routes(
                                    program,
                                    operators,
                                    exact_integer_casts,
                                    semantic,
                                    flow,
                                    state_flow,
                                    call_flow,
                                    &call_site,
                                    target_state_symbol,
                                    target_parameters,
                                    target_parameter_names,
                                    buckets,
                                    contracts,
                                    content_conservation,
                                )
                            }
                            SelectedTargetCrashRoutes::Private(summary) => {
                                let substitution = call_argument_substitution(
                                    program,
                                    operators,
                                    semantic,
                                    flow,
                                    state_flow,
                                    call_flow,
                                    target_parameters,
                                    arguments,
                                    exact_integer_casts,
                                );
                                normalize_summary_buckets(
                                    summary
                                        .iter()
                                        .map(|bucket| bucket.substitute(&substitution))
                                        .collect(),
                                )
                            }
                            SelectedTargetCrashRoutes::Empty => Vec::new(),
                        };
                        let entry_conditions = entry_paths::EntryConditions::new(program, semantic, flow);
                        let path = entry_conditions.for_machine(state_flow.machine_symbol).at_call(call_flow);
                        surviving_summary = surviving_summary.into_iter().map(|bucket| bucket.on_path(&path)).collect();
                        if let Some(caller) = program.machine_by_symbol(state_flow.machine_symbol) {
                            entry_equations::refine(
                                program,
                                operators,
                                caller,
                                &mut surviving_summary,
                            );
                        }
                        let surviving_buckets = surviving_summary
                            .into_iter()
                            .filter_map(SummaryCrashBucket::into_checked)
                            .collect::<Vec<_>>();
                        state_calls.push((
                            state_flow.machine_symbol,
                            checked_trees::CheckedCrashCallSite::new_with_commitment(
                                checked_trees::CrashCallSiteLocation::new(
                                    state_flow.state_symbol,
                                    u32::try_from(call_flow.statement_index).expect(
                                        "statement ordinal exceeds checked crash-call identity range",
                                    ),
                                    u32::try_from(call_flow.call_ordinal).expect(
                                        "call ordinal exceeds checked crash-call identity range",
                                    ),
                                ),
                                target_machine_symbol,
                                target_state_symbol,
                                target_contract_fingerprint,
                                target_contract_commitment,
                                surviving_buckets,
                            ),
                        ));
                    }
                    state_calls
                })
                .collect()
        });
    let mut calls_by_caller =
        symbols::SymbolKeyMap::<SymbolHandle, Vec<checked_trees::CheckedCrashCallSite>>::default();
    for state_calls in per_state_calls {
        for (caller_machine, site) in state_calls {
            calls_by_caller
                .entry(caller_machine)
                .or_default()
                .push(site);
        }
    }

    for plan in plans {
        let checked_calls = calls_by_caller.remove(&plan.machine).unwrap_or_default();
        // Publish the fixed point already used for call refinement. A body
        // governed only by published routes has no inferred row here.
        let body_summary = program.machine_by_symbol(plan.machine).and_then(|machine| {
            summary_by_machine.get(&plan.machine).map(|buckets| {
                checked_trees::CheckedBodyCrashSummary::new(
                    machine.symbol,
                    machine.states,
                    buckets
                        .iter()
                        .cloned()
                        .filter_map(SummaryCrashBucket::into_checked)
                        .collect(),
                )
            })
        });
        plan.crash = plan
            .crash
            .clone()
            .with_checked_calls(checked_calls)
            .expect("one checked crash-call record occupies each invocation coordinate")
            .with_checked_body_summary(body_summary);
    }
    inferred_body_summaries
}

pub(crate) fn infer_checked_machine_crash_causes(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    machine: SymbolHandle,
) -> Option<Vec<checked_trees::CrashCause>> {
    let mut machines = program
        .machines()
        .iter()
        .filter(|candidate| candidate.symbol == machine);
    let source = machines.next()?;
    if machines.next().is_some() {
        return None;
    }
    let mut plans = facts
        .contract_plans
        .machines
        .iter()
        .filter(|plan| plan.machine == machine);
    let plan = plans.next()?;
    if plans.next().is_some() {
        return None;
    }
    let summary = plan.crash.checked_body_summary(machine, source.states)?;
    Some(body_summary_causes(summary))
}

pub(crate) fn infer_checked_crash_causes(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
) -> Vec<(SymbolHandle, Vec<checked_trees::CrashCause>)> {
    // Sort borrowed rows once so duplicate identities stay unknown without
    // building another symbol map or rescanning every declaration per plan.
    let key = |symbol: SymbolHandle| (symbol.arena_index(), symbol.generation());
    let mut machines = program.machines().iter().collect::<Vec<_>>();
    machines.sort_unstable_by_key(|machine| key(machine.symbol));
    let mut plans = facts.contract_plans.machines.iter().collect::<Vec<_>>();
    plans.sort_unstable_by_key(|plan| key(plan.machine));
    plans
        .iter()
        .enumerate()
        .filter_map(|(position, plan)| {
            if position > 0 && plans[position - 1].machine == plan.machine
                || plans
                    .get(position + 1)
                    .is_some_and(|next| next.machine == plan.machine)
            {
                return None;
            }
            let position = machines
                .binary_search_by_key(&key(plan.machine), |machine| key(machine.symbol))
                .ok()?;
            let machine = machines[position];
            if position > 0 && machines[position - 1].symbol == machine.symbol
                || machines
                    .get(position + 1)
                    .is_some_and(|next| next.symbol == machine.symbol)
            {
                return None;
            }
            let summary = plan
                .crash
                .checked_body_summary(machine.symbol, machine.states)?;
            Some((machine.symbol, body_summary_causes(summary)))
        })
        .collect()
}

fn body_summary_causes(
    summary: &checked_trees::CheckedBodyCrashSummary,
) -> Vec<checked_trees::CrashCause> {
    let mut causes = summary
        .buckets()
        .iter()
        .map(checked_trees::CrashRouteBucket::cause)
        .collect::<Vec<_>>();
    causes.sort_unstable();
    causes.dedup();
    causes
}

/// The member name a produced entry operand carries. A case payload field
/// keeps its `Variant::field` qualification — the retained `case_variant`
/// spelling when present, else the field's declaring variant — so two
/// same-named payload fields of different cases cannot collide in the caller
/// namespace.
pub(super) fn member_entry_name(
    program: &dyn ::validation::CheckingInput,
    member: &typed_trees::expression::TableMemberExpression,
    field_symbol: symbols::SymbolHandle,
) -> String {
    field_entry_name(
        program,
        field_symbol,
        member.member.as_str(),
        member.case_variant.as_ref().map(|case| case.as_str()),
    )
}

pub(super) fn field_entry_name(
    program: &dyn ::validation::CheckingInput,
    field_symbol: symbols::SymbolHandle,
    field_name: &str,
    authored_case: Option<&str>,
) -> String {
    let Some(variant) = facts::payload_variant_for_field(program, field_symbol) else {
        return field_name.to_owned();
    };
    let case = authored_case.unwrap_or_else(|| program.symbols.name(variant));
    format!("{case}::{field_name}")
}
