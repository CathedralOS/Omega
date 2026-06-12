//! Lifetimes stage 1: output-borrow elision over machine declarations.
//!
//! A machine returning a view (`-> &T`, `-> &mut T`, `-> &string`, slice
//! views) must have an unambiguous source for the returned borrow:
//!
//! - a `&self`/`&mut self` parameter links the output to self (Rust's elision
//!   rule 3), regardless of other ref inputs;
//! - otherwise EXACTLY ONE ref input is the source (elision rule 1);
//! - two or more non-self ref inputs are ambiguous and rejected here, at the
//!   declaration, because explicit lifetime parameters (the tick spelling of
//!   frozen decision 15) are not implemented yet.
//!
//! Zero ref inputs with a view output is left alone (historical behavior);
//! such a machine has nothing the checker could link the output to, and the
//! conservative treatment of that shape is a later stage's concern.

use omega_core::diagnostics::Diagnostic;

pub(super) fn check_view_return_elision(
    program: &omega_typed_trees::TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if !is_reference_type(program, state.return_type) {
                continue;
            }

            let parameters = program.state_parameters(state);
            if parameters.iter().any(|parameter| parameter.is_self) {
                // Elision rule 3: the returned view borrows self.
                continue;
            }

            let candidates: Vec<&str> = parameters
                .iter()
                .filter(|parameter| is_reference_type(program, parameter.type_reference))
                .map(|parameter| parameter.name.as_str())
                .collect();
            if candidates.len() < 2 {
                continue;
            }

            // For a free machine the interesting name is the machine's; an
            // inner named state is more precise when it differs.
            let subject = if state.name.as_str() == "entry"
                || machine.name.as_str().ends_with(state.name.as_str())
            {
                machine.name.as_str()
            } else {
                state.name.as_str()
            };

            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` returns a view but takes {} candidate ref inputs ({}); cannot infer which input the returned view borrows; explicit lifetime parameters are not implemented yet",
                subject,
                candidates.len(),
                candidates
                    .iter()
                    .map(|name| format!("`{name}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
            )));
        }
    }
}

fn is_reference_type(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { .. } => true,
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            is_reference_type(program, *base_type)
        }
        _ => false,
    }
}
