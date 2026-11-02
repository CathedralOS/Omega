//! Runtime exit reasoning consumes the source-complete rank judgment, never
//! re-proves descent using the recursive result it is about to observe.
//! Keep licenses on exact tail occurrences; component membership alone licenses
//! nothing. Normal call preparation still proves every entry premise before
//! importing a licensed call's guarantees.

use super::{StructuralJudge, SymbolHandle};
use checked_trees::CheckedRankArrivalPlan;
use typed_trees::expression::ExpressionHandle;

#[derive(Default)]
pub(super) struct RuntimeInduction {
    calls: Vec<(SymbolHandle, ExpressionHandle, SymbolHandle)>,
}

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn retain_runtime_induction(
        &mut self,
        ranks: &CheckedRankArrivalPlan,
    ) {
        let program = self.program;
        let mut calls = Vec::new();
        for (_, component) in ranks.runtime_components.iter() {
            let members = ranks.runtime_members.span_or_empty(component.members);
            if !component.diagnostic.is_empty()
                || !members
                    .iter()
                    .any(|member| member.machine == self.machine_symbol)
                || !members.iter().all(|member| {
                    program
                        .machine(member.machine)
                        .is_some_and(|machine| machine.states == member.body)
                })
            {
                continue;
            }
            let edges = ranks.runtime_edges.span_or_empty(component.edges);
            for edge in edges.iter().filter(|edge| edge.strict) {
                // The retained component compresses the source target handle.
                // Ambiguous edges at one statement cannot authenticate a call.
                if edges
                    .iter()
                    .filter(|other| {
                        other.source == edge.source
                            && other.target == edge.target
                            && other.statement_ordinal == edge.statement_ordinal
                    })
                    .count()
                    != 1
                {
                    continue;
                }
                let Some(machine) = program.machine_holding_state(edge.source) else {
                    continue;
                };
                let Some(state) = program.state_in_machine(machine, edge.source) else {
                    continue;
                };
                let Ok(statement) = usize::try_from(edge.statement_ordinal) else {
                    continue;
                };
                for (ordinal, site) in
                    crate::collect_call_sites_in_statement(program, machine, state, statement)
                        .into_iter()
                        .enumerate()
                {
                    let crate::CallSite::Expression { expression, call } = site else {
                        continue;
                    };
                    if !call.selects_only_nominal_route() {
                        continue;
                    }
                    let Some((callee, entry)) =
                        crate::find_machine_by_entry_state(program, edge.target)
                    else {
                        continue;
                    };
                    if !members.iter().any(|member| member.machine == callee.symbol)
                        || crate::call_target_state(program, site.target_symbol(program))
                            .map(|state| state.symbol)
                            != Some(entry.symbol)
                    {
                        continue;
                    }
                    let Some(target) =
                        crate::transition_call_target(program, machine, state, statement, ordinal)
                    else {
                        continue;
                    };
                    if target.is_valid() && site.completes_transition(program, target) {
                        calls.push((state.symbol, expression, entry.symbol));
                    }
                }
            }
        }
        self.runtime_induction = std::sync::Arc::new(RuntimeInduction { calls });
    }

    pub(in crate::proof_contracts::contract_entailment) fn has_runtime_induction(
        &self,
        state: SymbolHandle,
        expression: ExpressionHandle,
        callee: SymbolHandle,
    ) -> bool {
        self.runtime_induction
            .calls
            .contains(&(state, expression, callee))
    }
}
