mod graph;
mod order;
mod ranking;

use crate::labels::machine_name;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::expression::{BinaryOperator, ExpressionNode};

pub(crate) fn check_machine_termination(
    program: &omega_typed_trees::TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    // The use-site subtraction spelling `decreases upper - lower` is retired:
    // the ranked subjects are spelled as the argumented tuple
    // `decreases (lower, upper) -> Nat::BoundedDistance` (the arrow's left
    // side is uniformly the ranked subjects; the named view receives them as
    // arguments in order). This fires for every machine carrying the retired
    // shape, terminating or not, so the old spelling cannot linger inertly.
    for machine in program.machines() {
        if let Some(message) = retired_subtraction_message(program, machine) {
            diagnostics.push(Diagnostic::error(message));
        }
    }

    for machine in program
        .machines()
        .iter()
        .filter(|machine| machine.terminates)
    {
        // A retired-spelling machine already has its directed diagnostic; the
        // ranking checks below would only stack a misleading "cannot prove".
        if retired_subtraction_message(program, machine).is_some() {
            continue;
        }

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
            ranking::DecreaseOutcome::InvertedDistance(inverted) => {
                diagnostics.push(Diagnostic::error(inverted_distance_message(
                    &machine_name(program, machine.symbol),
                    &inverted,
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
             (builtin views: Nat::Descending, Nat::BoundedDistance, Slice::Length)"
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

/// Render the diagnostic for a two-subject `decreases` tuple whose subjects
/// are inverted: the swapped subjects prove as the named bounded distance, so
/// the message names the ranking and the corrected spelling instead of a bare
/// "cannot prove".
fn inverted_distance_message(machine: &str, inverted: &ranking::InvertedDistance) -> String {
    let declared = inverted.declared.as_str();
    let corrected = inverted.corrected.as_str();
    format!(
        "cannot prove decreases clause for terminating machine {machine}: \
         `decreases {declared}` inverts the named bounded distance -- \
         `Nat::BoundedDistance` ranks `(lower, upper)`, which descends as the \
         lower value climbs; write `decreases {corrected} -> Nat::BoundedDistance`"
    )
}

/// The retirement diagnostic for the use-site subtraction spelling, or `None`
/// when the machine's decreases clause is not a single top-level subtraction.
/// The message spells the exact argumented replacement, with the subtraction's
/// operands reordered into the view's `(lower, upper)` parameter order.
fn retired_subtraction_message(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
) -> Option<String> {
    let [decreases] = program
        .expression_table
        .expression_handles(machine.decreases)
    else {
        return None;
    };
    let ExpressionNode::Binary(binary) = program.expression_table.expression(*decreases) else {
        return None;
    };
    if !matches!(binary.operator, BinaryOperator::Subtract) {
        return None;
    }
    let upper = order::decreasing_value_text(program, binary.left);
    let lower = order::decreasing_value_text(program, binary.right);
    Some(format!(
        "the use-site subtraction `decreases {upper} - {lower}` on machine {} is retired: \
         spell the ranking as `decreases ({lower}, {upper}) -> Nat::BoundedDistance` \
         (the tuple lists the ranked subjects, bound in order to the view's \
         (lower, upper) parameters)",
        machine_name(program, machine.symbol)
    ))
}
