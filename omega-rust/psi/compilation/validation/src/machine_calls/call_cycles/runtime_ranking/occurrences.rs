//! Runtime component authority belongs to an exact executable occurrence.
//! Prefix frame preservation permits reading a rank after an acyclic call; it
//! does not make that call a tail edge. Check the canonical call inventory
//! before the prefix walk can skip a preserving statement. A classified tail
//! to the same destination cannot cover a call in a prefix, guard or argument.

use crate::machine_calls::calls::{collect_call_sites_in_statement, transition_call_target};

pub(super) fn require_classified_tail_occurrences(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_ordinal: usize,
    component: &[usize],
    dispatched: &[(symbols::SymbolHandle, Vec<usize>)],
) -> Result<(), &'static str> {
    for (call_ordinal, call) in
        collect_call_sites_in_statement(program, machine, state, statement_ordinal)
            .into_iter()
            .enumerate()
    {
        let target = call.target_symbol(program);
        // A selector call is one occurrence that can enter every selected
        // implementation. It owes the same tail classification as a direct
        // call into this component; the rank judgment then reads the data
        // operands after the selector.
        if let Some((_, implementations)) = dispatched.iter().find(|(slot, _)| *slot == target)
            && implementations
                .iter()
                .any(|callee| component.contains(callee))
        {
            let Some(target_handle) =
                transition_call_target(program, machine, state, statement_ordinal, call_ordinal)
            else {
                return Err("an internal call occurrence lacks a classified tail edge");
            };
            if !target_handle.is_valid() || !call.completes_transition(program, target_handle) {
                return Err("an internal call occurrence lacks a classified tail edge");
            }
            continue;
        }
        let Some(callee) = super::target_machine(program, target) else {
            continue;
        };
        if !component.contains(&callee)
            || (program.machines()[callee].symbol == machine.symbol
                && super::internal_state_index(program, machine, target).is_some())
        {
            continue;
        }
        let Some(target_handle) =
            transition_call_target(program, machine, state, statement_ordinal, call_ordinal)
        else {
            return Err("an internal call occurrence lacks a classified tail edge");
        };
        // Guard calls deliberately carry the absence handle: they belong to
        // this transition's evaluation but not to either target operand.
        if !target_handle.is_valid() {
            return Err("an internal call occurrence lacks a classified tail edge");
        }
        let is_tail_root = call.completes_transition(program, target_handle);
        if !is_tail_root {
            return Err("an internal call occurrence lacks a classified tail edge");
        }
    }
    Ok(())
}
