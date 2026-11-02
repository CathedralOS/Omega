//! Case tags survive scalar writes, not replacement of a structural subject.
//!
//! The runtime case judge retains tags for entry parameters, saved call results
//! and branch subjects. Establish preservation for the whole reachable body,
//! rather than checking only the caller's outward frame: writes to an owned
//! local can disappear from that frame while still invalidating its saved tag.
//! Complete frames exclude unknown effects; checked stored-value types
//! distinguish scalar mutation (including atomic events) from replacement of
//! a case-bearing value or reference binding. This deliberately proves no
//! scalar equations. Non-case requirements keep the stable-value judgment.

use checked_trees::CheckFacts;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;

pub(super) fn is_case_requirement(
    program: &dyn validation::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> bool {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    if binary.operator == BinaryOperator::And {
        return is_case_requirement(program, machine, state, binary.left)
            && is_case_requirement(program, machine, state, binary.right);
    }
    validation::has_exact_case_membership_meaning(program, machine, Some(state), expression, binary)
}

pub(super) fn body_preserves_case_tags(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: &Machine,
) -> bool {
    let mut pending = vec![machine.symbol];
    let mut visited: Vec<SymbolHandle> = Vec::new();
    while let Some(symbol) = pending.pop() {
        if visited.contains(&symbol) {
            continue;
        }
        visited.push(symbol);
        let Some(machine) = program.machine_by_symbol(symbol) else {
            return false;
        };
        if !machine.supplies_checked_body() {
            return false;
        }
        let Some(mutation) = facts.mutation.for_machine(symbol) else {
            return false;
        };
        if program.machine_states(machine).is_empty() {
            return false;
        }
        for state in program.machine_states(machine) {
            if !mutation
                .state_write_frames
                .iter()
                .any(|frame| frame.state == state.symbol && frame.frame.is_complete())
            {
                return false;
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let Some(assignment) = statement.as_assignment() {
                    // A scalar referent store preserves tags even though its
                    // destination is declared as a reference. Descriptor
                    // replacement still stores a reference, not its referent.
                    let scalar_destination = validation::assignment_stored_value_type(
                        program, machine, state, assignment,
                    )
                    .and_then(|reference| program.primitive_type_reference(reference))
                    .is_some();
                    if !scalar_destination {
                        return false;
                    }
                }
            }
        }
        for (_, state) in facts
            .flow
            .control
            .states
            .iter()
            .filter(|(_, state)| state.machine_symbol == symbol)
        {
            for call in facts.flow.control.calls.span_or_empty(state.calls) {
                let callee = program
                    .machine_by_symbol(call.target_symbol)
                    .or_else(|| program.machine_holding_state(call.target_symbol));
                let Some(callee) = callee else {
                    // An unresolved/boundary effect cannot establish that all
                    // retained subjects remain the same structural revision.
                    return false;
                };
                pending.push(callee.symbol);
            }
        }
    }
    true
}
