mod calls;
mod direct;
mod domains;
mod evaluator;
mod exits;
mod writes;
// `pub(super)` so the operator-`requires` discharge (checks/operators) can
// reuse the domain-derived boolean proving labels.
pub(super) mod labels;
mod places;
mod prover;

use calls::check_call_requires;
use exits::check_exit_ensures;
use omega_checked_trees::CheckFacts;
use omega_core::diagnostics::Diagnostic;
use writes::check_domain_field_writes;

pub(crate) fn check_flow_call_contracts(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for (_, state_flow) in facts.flow.control.states.iter() {
        for call_flow in facts.flow.control.calls.span_or_empty(state_flow.calls) {
            check_call_requires(program, facts, state_flow, call_flow, &mut diagnostics);
        }
        for exit_flow in facts.flow.control.exits.span_or_empty(state_flow.exits) {
            check_exit_ensures(program, facts, state_flow, exit_flow, &mut diagnostics);
        }
        // #66 write-enforcement: every assignment into a domain-refined field must
        // establish the value in that domain (the soundness floor for trusting the
        // field's declared domain on read).
        check_domain_field_writes(program, facts, state_flow, &mut diagnostics);
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
