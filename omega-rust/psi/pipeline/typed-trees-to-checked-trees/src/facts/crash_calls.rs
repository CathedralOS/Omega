//! Ordinary private crash summaries compose through exact invocation substitutions;
//! authored published ceilings remain authoritative. Predicate reduction requires
//! the callee's builtin operator meaning and entry custody for referenced actuals.
//! Optional scalar annotations carry checked lowering evidence: after a fully
//! concrete substitution they may discharge or confirm a route the domain-free
//! identity cannot fold, and they never keep an undecidable origin from widening.
//!
//! This file attaches checked crash calls and infers crash causes.
//! `summary_predicates.rs` carries summary crash predicates and buckets,
//! `route_substitution.rs` substitutes call arguments into published crash
//! routes and `private_summaries.rs` solves private body summaries;
//! `tests.rs` holds the crash call tests.

mod private_summaries;
mod route_substitution;
mod summary_predicates;
#[cfg(test)]
mod tests;

use crate::facts::crash_calls::private_summaries::{
    infer_private_body_summaries, requirement_signature,
};
use crate::facts::crash_calls::route_substitution::{
    SelectedTargetCrashRoutes, call_argument_substitution, refine_published_crash_routes,
};
use crate::facts::crash_calls::summary_predicates::{
    SummaryCrashBucket, normalize_summary_buckets,
};
use checked_trees::CrashPredicateExpression;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

pub(super) fn crash_predicate_from_expression(
    program: &TypedTrees,
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
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => CrashPredicateExpression::Binary {
            operator: binary.operator as u8,
            left: Box::new(crash_predicate_from_expression(
                program,
                binary.left,
                parameter_names,
                content_conservation,
            )),
            right: Box::new(crash_predicate_from_expression(
                program,
                binary.right,
                parameter_names,
                content_conservation,
            )),
        },
        ExpressionNode::Unary(unary) => CrashPredicateExpression::Unary {
            operator: unary.operator as u8,
            operand: Box::new(crash_predicate_from_expression(
                program,
                unary.operand,
                parameter_names,
                content_conservation,
            )),
        },
        ExpressionNode::Integer(value) => {
            CrashPredicateExpression::Integer(value.text().to_owned())
        }
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
            receiver: Box::new(crash_predicate_from_expression(
                program,
                member.receiver,
                parameter_names,
                content_conservation,
            )),
            member: member.member.as_str().to_owned(),
        },
        // A `collection[index]` read keeps both children structured: either
        // may carry a formal, and substitution must reach each rather than
        // hide it inside a flattened `Opaque` display. A range `start..end`
        // index has no structured variant yet — it stays `Opaque`, which
        // keeps refusing substitution.
        ExpressionNode::Indexed(indexed) => CrashPredicateExpression::Indexed {
            collection: Box::new(crash_predicate_from_expression(
                program,
                indexed.collection,
                parameter_names,
                content_conservation,
            )),
            index: Box::new(crash_predicate_from_expression(
                program,
                indexed.index,
                parameter_names,
                content_conservation,
            )),
        },
        ExpressionNode::Call(call) => CrashPredicateExpression::Call {
            target: call.target.as_str().to_owned(),
            receiver: Box::new(crash_predicate_from_expression(
                program,
                call.receiver,
                parameter_names,
                content_conservation,
            )),
            arguments: program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| {
                    crash_predicate_from_expression(
                        program,
                        *argument,
                        parameter_names,
                        content_conservation,
                    )
                })
                .collect(),
        },
        other => {
            let _ = other;
            CrashPredicateExpression::Opaque(program.expression_table.display_name(expression))
        }
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
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    semantic: &facts::FactPlan,
    flow: &checked_trees::FlowFacts,
    content_conservation: &[validation::ContentConservationSourcePlan],
    crash_capsules: &[checked_trees::CrashContractCapsule],
    plans: &mut [checked_trees::MachineContractPlan],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) {
    let inferred_body_summaries = infer_private_body_summaries(
        program,
        operators,
        exact_integer_casts,
        semantic,
        flow,
        content_conservation,
        crash_capsules,
        plans,
        call_frames,
    );
    let mut calls_by_caller =
        Vec::<(SymbolHandle, Vec<checked_trees::CheckedCrashCallSite>)>::new();
    for (_, state_flow) in flow.control.states.iter() {
        for call_flow in flow.control.calls.span_or_empty(state_flow.calls) {
            let Some((target_machine_symbol, target_state_symbol)) =
                crate::proof::contract_target_from_state_symbol(program, call_flow.target_symbol)
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
                target_routes,
                target_contract_fingerprint,
                target_contract_commitment,
            ) = if let (Some(target_machine), Some(target_plan)) = (local_target, local_plan) {
                let Some(target_state) = program
                    .machine_states(target_machine)
                    .iter()
                    .find(|state| state.symbol == target_state_symbol)
                else {
                    continue;
                };
                let target_routes = if !target_plan.crash.published().is_empty() {
                    SelectedTargetCrashRoutes::Published {
                        buckets: target_plan.crash.published(),
                        contracts: program.machine_contracts(target_machine),
                    }
                } else if target_machine.supply_mode
                    == language_semantics::MachineSupplyMode::CheckedBody
                {
                    let Some((_, summary)) = inferred_body_summaries
                        .iter()
                        .find(|(machine, _)| *machine == target_machine_symbol)
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
                    target_routes,
                    target_plan.report_fingerprint,
                    target_plan.commitment,
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
            let Some(call_site) = crate::semantic_calls::find_call_site(
                program,
                state_flow.machine_symbol,
                state_flow.state_symbol,
                call_flow.statement_index,
                call_flow.call_ordinal,
            ) else {
                continue;
            };
            if matches!(
                call_site,
                crate::semantic_calls::CallSite::TransitionNamed { .. }
            ) {
                // A named transition transfers within the current machine; it
                // is not an invocation of that machine's public crash ceiling.
                continue;
            }
            let arguments =
                crate::semantic_calls::call_site_argument_expressions(program, &call_site);
            let surviving_summary = match target_routes {
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
                        &target_parameter_names,
                        buckets,
                        contracts,
                        content_conservation,
                        call_frames,
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
                    // This call still has its invocation context, so a
                    // member-projected leaf resolves the argument's entry
                    // operand at the read projection instead of widening the
                    // route to `Truth` when a sibling field was written.
                    let mut resolve_entry = |ordinal: u32, members: &[String]| {
                        crate::facts::crash_calls::route_substitution::call_argument_entry_operand(
                            program,
                            state_flow.machine_symbol,
                            state_flow.state_symbol,
                            call_flow.statement_index,
                            target_parameters,
                            arguments,
                            &substitution.identity,
                            ordinal,
                            members,
                        )
                    };
                    normalize_summary_buckets(
                        summary
                            .iter()
                            .map(|bucket| {
                                bucket.substitute_with_entry_resolver(
                                    &substitution,
                                    Some(&mut resolve_entry),
                                )
                            })
                            .collect(),
                    )
                }
                SelectedTargetCrashRoutes::Empty => Vec::new(),
            };
            let surviving_buckets = surviving_summary
                .into_iter()
                .filter_map(SummaryCrashBucket::into_checked)
                .collect::<Vec<_>>();
            let caller_index = calls_by_caller
                .iter()
                .position(|(machine, _)| *machine == state_flow.machine_symbol)
                .unwrap_or_else(|| {
                    calls_by_caller.push((state_flow.machine_symbol, Vec::new()));
                    calls_by_caller.len() - 1
                });
            calls_by_caller[caller_index].1.push(
                checked_trees::CheckedCrashCallSite::new_with_commitment(
                    checked_trees::CrashCallSiteLocation::new(
                        state_flow.state_symbol,
                        u32::try_from(call_flow.statement_index)
                            .expect("statement ordinal exceeds checked crash-call identity range"),
                        u32::try_from(call_flow.call_ordinal)
                            .expect("call ordinal exceeds checked crash-call identity range"),
                    ),
                    target_machine_symbol,
                    target_state_symbol,
                    target_contract_fingerprint,
                    target_contract_commitment,
                    surviving_buckets,
                ),
            );
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

pub(crate) fn infer_checked_machine_crash_causes(
    program: &TypedTrees,
    facts: &checked_trees::CheckFacts,
    machine: SymbolHandle,
) -> Option<Vec<checked_trees::CrashCause>> {
    let mut matching = infer_checked_crash_causes(program, facts)
        .into_iter()
        .filter(|(candidate, _)| *candidate == machine);
    let (_, causes) = matching.next()?;
    matching.next().is_none().then_some(causes)
}

pub(crate) fn infer_checked_crash_causes(
    program: &TypedTrees,
    facts: &checked_trees::CheckFacts,
) -> Vec<(SymbolHandle, Vec<checked_trees::CrashCause>)> {
    let content_conservation = validation::build_content_conservation_plans(program);
    // Validation-only exact-cast facts are not retained in CheckedTrees. They
    // feed only CallArgumentSubstitution.scalar, never its identity. Guard
    // retention, equality and fixed-point closure still use the identity
    // alone; a scalar annotation decides only a retained guard whose
    // substituted identity cannot fold, through a closed integer comparison.
    // This query does not publish the guard annotations it consumes.
    let summaries = infer_private_body_summaries(
        program,
        &facts.operators,
        &[],
        &facts.semantic,
        &facts.flow,
        &content_conservation,
        &facts.contract_plans.crash_capsules,
        &facts.contract_plans.machines,
        None,
    );
    summaries
        .into_iter()
        .filter(|(machine, _)| {
            program
                .machines()
                .iter()
                .filter(|candidate| candidate.symbol == *machine)
                .count()
                == 1
                && facts
                    .contract_plans
                    .machines
                    .iter()
                    .filter(|candidate| candidate.machine == *machine)
                    .count()
                    == 1
        })
        .map(|(machine, buckets)| {
            let mut causes = buckets
                .into_iter()
                .map(|bucket| bucket.cause)
                .collect::<Vec<_>>();
            causes.sort_unstable();
            causes.dedup();
            (machine, causes)
        })
        .collect()
}
