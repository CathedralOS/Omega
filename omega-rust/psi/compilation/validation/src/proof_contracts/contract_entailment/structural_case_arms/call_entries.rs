//! Observe private body occurrences at ordered call entry, before completion.
//!
//! A returned value may be hoisted into an ordinary local by lowering. Its
//! callee still contains the same source occurrences. Statement ownership and
//! normal operation entries preserve earlier siblings, effects and snapshots;
//! evaluating a detached call expression would lose that context. This walk
//! observes the callee without importing its own uncompleted guarantees.

use super::*;
use crate::proof_contracts::contract_entailment::specification_calls::{
    SelectedCallContract, selected_call_site_contract,
};
use crate::proof_contracts::contract_entailment::structural_judgment::normal_returns::{
    NormalOperationEntry, OperationOperandDemand,
};
use std::collections::{BTreeMap, BTreeSet};

/// A refuted arm excludes its complete target, including nested call operands.
/// The ordinary occurrence owner separates that target from the live guard
/// and sibling arm; source order and spelling supply no absence evidence.
pub(super) fn exclude_target_calls(
    program: &dyn crate::CheckingInput,
    invocation: &Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    target: typed_trees::statement::TransitionTargetHandle,
    visitors: &mut [&mut (dyn StructuralPrefixVisitor + '_)],
) {
    if !target.is_valid() {
        return;
    }
    for occurrence in crate::machine_calls::calls::collect_statement_call_occurrences(
        program,
        invocation,
        state,
        statement_index,
    ) {
        if occurrence.transition_target != target {
            continue;
        }
        let site = occurrence.site;
        let Some(SelectedCallContract::Concrete(_, entry)) =
            selected_call_site_contract(program, invocation.symbol, site)
        else {
            continue;
        };
        let expression = match site {
            crate::CallSite::Expression { expression, .. } => Some(expression),
            _ => None,
        };
        for visitor in visitors.iter_mut() {
            visitor.excluded_body(
                state.symbol,
                statement_index,
                expression,
                target,
                entry.symbol,
            );
        }
    }
}

/// A selected target starts after its completed guard. Exact source lane
/// association keeps guard and sibling operands out of this entry inventory.
#[derive(Clone, Copy)]
pub(super) enum CallEntryScope {
    Statement,
    SelectedTarget(typed_trees::statement::TransitionTargetHandle),
}

#[allow(clippy::too_many_arguments)]
pub(super) fn observe_calls<'program>(
    program: &'program dyn crate::CheckingInput,
    theorem: &Machine,
    invocation: &Machine,
    judge: &StructuralJudge<'program>,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
    states: &[typed_trees::state::State],
    state: &typed_trees::state::State,
    statement_index: usize,
    scope: CallEntryScope,
    parameter_names: &[String],
    environment: &[(String, StructuralTerm)],
    hypotheses: &[(String, StructuralTerm)],
    equations: &[(StructuralTerm, StructuralTerm)],
    citations: &[PendingStructuralCitation],
    path: &mut Vec<SymbolHandle>,
    arrivals: &mut StructuralArrivalLog<'program>,
    arrival_parent: usize,
    fresh: &mut usize,
    visitors: &mut [&mut (dyn StructuralPrefixVisitor + '_)],
    guarantees: &CitationFacts,
) -> Option<()> {
    if let CallEntryScope::SelectedTarget(target) = scope {
        let transition = program
            .statement_table
            .statement_at(state, statement_index)?
            .as_transition()?;
        if !target.is_valid()
            || !program.statement_table.transition_target_is_valid(target)
            || !transition.targets().any(|candidate| candidate == target)
        {
            return None;
        }
    }
    // Only members observing inside another body ride a call entry. A site
    // in this state or on the current path is already (or can only still be)
    // served by this frame's own statements.
    let mut active: Vec<&mut (dyn StructuralPrefixVisitor + '_)> = visitors
        .iter_mut()
        .filter_map(|member| {
            (member.site().state != state.symbol && !path.contains(&member.site().state))
                .then_some(&mut **member)
        })
        .collect();
    if active.is_empty() {
        return Some(());
    }
    let mut pending = Vec::new();
    let mut operations = Vec::new();
    for occurrence in crate::machine_calls::calls::collect_statement_call_occurrences(
        program,
        invocation,
        state,
        statement_index,
    ) {
        if let CallEntryScope::SelectedTarget(target) = scope
            && occurrence.transition_target != target
        {
            continue;
        }
        let Some(SelectedCallContract::Concrete(callee, entry)) =
            selected_call_site_contract(program, invocation.symbol, occurrence.site)
        else {
            continue;
        };
        let eligible = active
            .iter()
            .enumerate()
            .filter_map(|(position, member)| {
                (!path.contains(&entry.symbol)
                    && prefix_site_is_reachable(program, entry, &**member))
                .then_some(position)
            })
            .collect::<Vec<_>>();
        if eligible.is_empty() {
            continue;
        }
        if !body_can_be_observed(
            program,
            callee,
            classification,
            judge.requires_value_preservation(),
        ) {
            return None;
        }
        if let crate::CallSite::Expression { expression, .. } = occurrence.site {
            operations.push(expression);
        }
        pending.push((occurrence, entry, eligible));
    }
    // One ordered prefix captures every requested pre-call frame. A missing
    // frame means unreachable only after that complete prefix succeeded.
    let mut observed = expression_entries(
        program,
        invocation,
        judge,
        state,
        statement_index,
        scope,
        environment,
        &operations,
    )?;
    for (occurrence, entry, eligible_indices) in pending {
        let site = occurrence.site;
        let mut eligible = active
            .iter_mut()
            .enumerate()
            .filter_map(|(position, member)| {
                eligible_indices
                    .binary_search(&position)
                    .is_ok()
                    .then_some(&mut **member)
            })
            .collect::<Vec<_>>();
        let parameters = program.state_parameters(entry);
        let expression = match site {
            crate::CallSite::Expression { expression, .. } => Some(expression),
            _ => None,
        };
        let frames = if let Some(expression) = expression {
            match observed.remove(&(expression.arena_index(), expression.generation())) {
                Some(entries) => entries,
                None => {
                    for member in eligible.iter_mut() {
                        member.excluded_body(
                            state.symbol,
                            statement_index,
                            Some(expression),
                            occurrence.transition_target,
                            entry.symbol,
                        );
                    }
                    continue;
                }
            }
        } else {
            judge.normal_argument_entries(
                crate::call_site_argument_expressions(program, &site),
                parameters,
                OperationOperandDemand::All,
                environment,
                (invocation, state, state.return_type),
                Some(site),
            )?
        };
        let subjects = if judge.requires_value_preservation() {
            let frames = judge.frame_resolver()?;
            match site {
                crate::CallSite::Expression { expression, .. } => {
                    frames.expression_entry_reference_subjects(invocation, expression, parameters)
                }
                crate::CallSite::Statement(call) => {
                    frames.statement_entry_reference_subjects(invocation, call, parameters)
                }
                _ => None,
            }
        } else {
            None
        }
        .unwrap_or_default();
        for frame in frames {
            let arguments = frame.observed_arguments()?;
            let (incoming, captured) = frame.judge.bind_invocation(
                entry,
                &arguments,
                &subjects,
                environment,
                (invocation, state, state.return_type),
            )?;
            for member in eligible.iter_mut() {
                member.entered_body(
                    state.symbol,
                    statement_index,
                    expression,
                    occurrence.transition_target,
                    entry.symbol,
                );
            }
            recognize_structural_state_leaves(
                program,
                theorem,
                &captured,
                classification,
                diagnostics,
                states,
                entry,
                parameter_names,
                incoming,
                hypotheses.to_vec(),
                equations.to_vec(),
                citations.to_vec(),
                true,
                path,
                arrivals,
                arrival_parent,
                fresh,
                eligible.as_mut_slice(),
                guarantees.clone(),
            )?;
        }
    }
    Some(())
}

/// Metadata chooses one authenticated schedule; the normal observation owner
/// alone evaluates it. Inventories of other targets are never execution order.
#[allow(clippy::too_many_arguments)]
fn expression_entries<'program>(
    program: &'program dyn crate::CheckingInput,
    invocation: &Machine,
    judge: &StructuralJudge<'program>,
    state: &typed_trees::state::State,
    statement_index: usize,
    scope: CallEntryScope,
    environment: &[(String, StructuralTerm)],
    operations: &[ExpressionHandle],
) -> Option<BTreeMap<(u32, u32), Vec<NormalOperationEntry<'program>>>> {
    let mut entries = BTreeMap::<_, Vec<_>>::new();
    if operations.is_empty() {
        return Some(entries);
    }
    let statement = program
        .statement_table
        .statement_at(state, statement_index)?;
    let anchor = match scope {
        CallEntryScope::Statement => {
            // Ordinary statement roots are ordered operands. This includes
            // assignment target capture before RHS and call actuals before
            // the outer invocation; the last root owns the complete prefix.
            if matches!(
                statement,
                typed_trees::statement::StatementNode::Transition(_)
            ) {
                return None;
            }
            program
                .tables
                .expression_roots(statement)
                .into_iter()
                .map(|(_, root)| root)
                .filter(|root| root.is_valid())
                .next_back()?
        }
        CallEntryScope::SelectedTarget(target) => {
            let typed_trees::statement::TransitionTargetNode::Value(expression) =
                program.statement_table.transition_target(target)
            else {
                return None;
            };
            *expression
        }
    };
    let observed = judge.completed_observations(
        statement_index,
        anchor,
        environment,
        (invocation, state, state.return_type),
        &[],
        operations,
    )?;
    // A partial prefix cannot prove absence, and earlier snapshots from only
    // some paths cannot stand in for all arrivals. Keep refusal distinct from
    // a completely explored branch which does not invoke a requested call.
    if !observed.roots_unavailable.is_empty() {
        return None;
    }
    let requested = operations
        .iter()
        .map(|expression| (expression.arena_index(), expression.generation()))
        .collect::<BTreeSet<_>>();
    for frame in observed.operations {
        let key = (
            frame.expression.arena_index(),
            frame.expression.generation(),
        );
        if requested.contains(&key) {
            entries.entry(key).or_default().push(frame);
        }
    }
    Some(entries)
}

/// Observing an executed prefix or normal completion does not treat the callee
/// as a repeatable mathematical function. Runtime observation retains ordered
/// storage effects; theorem and case-only traversal keep their existing policy.
pub(in crate::proof_contracts::contract_entailment) fn body_can_be_observed(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    preserve_runtime_values: bool,
) -> bool {
    if !machine.body_is_present
        || !machine.supply_mode.is_checked_body()
        || machine.target_family_interface
    {
        return false;
    }
    if preserve_runtime_values || classification.is_proof_machine(program, machine) {
        return true;
    }
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    let eligible = |operational: &flow_effects::OperationalPlan,
                    reaches: &flow_effects::ServiceReachInferencePlan| {
        let result = crate::machine_calls::denotational_calls::checked_value_call_candidate(
            program,
            machine,
            entry,
            operational,
            reaches,
        );
        if super::super::struct_trace()
            && let Err(reason) = &result
        {
            eprintln!(
                "STRUCT body opaque machine={} reason={reason:?}",
                machine.name
            );
        }
        result.is_ok()
    };
    if let Some(plans) = program.prepared_effect_plans() {
        return eligible(&plans.operational, &plans.service_reaches);
    }
    let operational = crate::machine_calls::effect_inference::infer_operational_may(program);
    let reaches =
        crate::machine_calls::effect_inference::infer_service_reaches(program, &operational);
    eligible(&operational, &reaches)
}
