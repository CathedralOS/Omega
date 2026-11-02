//! Computed ranking between authored ranked entries.
//!
//! Helpers need no synthetic rank. Removing ranked entries must leave a DAG,
//! so every infinite call path would cross infinitely many ranked entries.
//! Each such interval must strictly decrease the common well-founded measure.
//! The ordered prefix judgment checks all paths to each exact closing call.

use super::{
    ExactProofCallEdge, ValidatedProofRankInterval, ValidatedProofRecursiveCallSite,
    proof_rank_parameter, strongly_connected_components,
};
use crate::proof_contracts::contract_entailment::proof_interval_strict_decrease_judged;

pub(super) fn validate_component(
    program: &dyn crate::CheckingInput,
    members: &[usize],
    calls: &[ExactProofCallEdge],
) -> Result<Vec<ValidatedProofRankInterval>, &'static str> {
    let machines = program.machines();
    let mut ranks = Vec::new();
    for member in members {
        let machine = &machines[*member];
        let Some((symbol, _, identity)) = proof_rank_parameter(program, machine) else {
            if machine.termination_plan.implementation_witness.is_some() {
                return Err("an authored proof rank is not an entry parameter");
            }
            continue;
        };
        let entry = program
            .machine_states(machine)
            .first()
            .ok_or("ranked body has no entry")?;
        let (position, parameter) = program
            .state_parameters(entry)
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == symbol)
            .ok_or("rank parameter is absent from its entry")?;
        if parameter.is_mutable {
            return Err("a computed proof rank must retain its entry value");
        }
        ranks.push((*member, position, parameter, identity));
    }
    let Some((_, _, _, rank_identity)) = ranks.first() else {
        return Err("the proof component has no authored ranking entry");
    };
    if ranks
        .iter()
        .any(|(_, _, _, identity)| identity != rank_identity)
    {
        return Err("ranked entries do not share one well-founded carrier");
    }
    let is_ranked = |member| ranks.iter().any(|(candidate, ..)| *candidate == member);
    let mut unranked = vec![Vec::new(); machines.len()];
    for call in calls {
        if !is_ranked(call.caller) && !is_ranked(call.callee) {
            unranked[call.caller].push(call.callee);
        }
    }
    if strongly_connected_components(&unranked)
        .iter()
        .any(|component| {
            component.len() > 1
                || component
                    .first()
                    .is_some_and(|member| unranked[*member].contains(member))
        })
    {
        return Err("a helper-only cycle can avoid every ranked entry");
    }
    let ranked_entries = ranks
        .iter()
        .map(|(member, ..)| machines[*member].symbol)
        .collect::<Vec<_>>();
    let mut intervals = Vec::new();
    let mut common_relation = symbols::SymbolHandle::invalid();
    for (origin, _, measure, _) in &ranks {
        let mut pending = vec![*origin];
        let mut visited = Vec::new();
        while let Some(caller) = pending.pop() {
            if visited.contains(&caller) {
                continue;
            }
            visited.push(caller);
            for call in calls.iter().filter(|call| call.caller == caller) {
                let Some((_, rank_position, target_rank, _)) =
                    ranks.iter().find(|(member, ..)| *member == call.callee)
                else {
                    pending.push(call.callee);
                    continue;
                };
                let (state_symbol, statement_index) = match call.site {
                    ValidatedProofRecursiveCallSite::Statement {
                        state,
                        statement_index,
                    }
                    | ValidatedProofRecursiveCallSite::Expression {
                        state,
                        statement_index,
                        ..
                    }
                    | ValidatedProofRecursiveCallSite::Transition {
                        state,
                        statement_index,
                        ..
                    } => (state, statement_index),
                };
                let state = program
                    .state_in_machine(&machines[caller], state_symbol)
                    .ok_or("closing call is absent from its recorded body")?;
                let argument = *call
                    .arguments
                    .get(*rank_position)
                    .ok_or("closing call has no actual for the selected rank")?;
                let Some(relation) = proof_interval_strict_decrease_judged(
                    program,
                    &machines[*origin],
                    state,
                    statement_index,
                    argument,
                    measure,
                    &ranked_entries,
                ) else {
                    return Err(
                        "a computed closing argument is not proved smaller on every incoming path",
                    );
                };
                if common_relation.is_valid() && relation != common_relation {
                    return Err("computed intervals select different order relations");
                }
                common_relation = relation;
                intervals.push(ValidatedProofRankInterval {
                    origin: machines[*origin].symbol,
                    origin_rank: measure.symbol,
                    caller: machines[caller].symbol,
                    callee: machines[call.callee].symbol,
                    callee_rank: target_rank.symbol,
                    site: call.site,
                    argument,
                    relation,
                });
            }
        }
    }
    if intervals.is_empty() {
        return Err("no checked interval closes at a ranked entry");
    }
    Ok(intervals)
}
