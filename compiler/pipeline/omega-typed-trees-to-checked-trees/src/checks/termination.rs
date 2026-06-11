mod graph;
mod order;
mod ranking;

use crate::labels::machine_name;
use omega_core::diagnostics::Diagnostic;

pub(crate) fn check_machine_termination(
    program: &omega_typed_trees::TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for machine in program
        .machines()
        .iter()
        .filter(|machine| machine.terminates)
    {
        if !graph::machine_has_cycle(program, machine) {
            continue;
        }

        if machine.decreases.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "terminating machine {} contains a recursive cycle but has no decreases clause",
                machine_name(program, machine.symbol)
            )));
            continue;
        }

        match ranking::machine_decrease_outcome(program, machine) {
            ranking::DecreaseOutcome::Proven => {}
            ranking::DecreaseOutcome::Unproven => {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove decreases clause for terminating machine {}",
                    machine_name(program, machine.symbol)
                )));
            }
            ranking::DecreaseOutcome::AmbiguousOrder(ambiguity) => {
                diagnostics.push(Diagnostic::error(ambiguous_order_message(
                    &machine_name(program, machine.symbol),
                    &ambiguity,
                )));
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Render the diagnostic for a plain `decreases value` clause whose value has
/// no inferable builtin ranking: name the value, say why inference failed, and
/// suggest the explicit `-> View` form. Declared measures matching the value's
/// type are suggested by name but are never selected implicitly — even a single
/// declared measure requires the explicit form, so declaring a second measure
/// later cannot silently change distant `decreases` clauses.
fn ambiguous_order_message(machine: &str, ambiguity: &order::AmbiguousDefault) -> String {
    let clause = ambiguity.clause.as_str();
    let reason = match &ambiguity.reason {
        order::AmbiguityReason::SignedInteger => {
            "signed values have no default well-founded order".to_string()
        }
        order::AmbiguityReason::NoBuiltinOrder { type_name } => {
            format!("`{type_name}` has no builtin well-founded order")
        }
        order::AmbiguityReason::UnknownShape => {
            "the decreasing value has no single builtin well-founded order".to_string()
        }
    };
    let suggestion = match ambiguity.declared_measures.as_slice() {
        [] => format!(
            "select one with `decreases {clause} -> View` \
             (builtin views: Nat::Descending, Slice::Length)"
        ),
        [only] => format!(
            "declared measures are never selected implicitly; \
             select one with `decreases {clause} -> {only}`"
        ),
        many => format!(
            "declared measures are never selected implicitly; \
             select one with `decreases {clause} -> View` (declared measures: {})",
            many.join(", ")
        ),
    };
    format!(
        "cannot infer a ranking for `decreases {clause}` in terminating machine {machine}: \
         {reason} -- {suggestion}"
    )
}
