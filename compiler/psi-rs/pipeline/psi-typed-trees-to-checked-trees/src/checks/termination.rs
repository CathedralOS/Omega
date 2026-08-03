mod graph;
mod order;
mod ranking;

use crate::labels::machine_name;
use psi_diagnostics::Diagnostic;

pub(crate) fn check_machine_termination(
    program: &psi_typed_trees::TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    // The use-site subtraction spelling `terminates by upper - lower` is retired:
    // the ranked subjects are spelled as the argumented tuple
    // `terminates by (lower, upper) -> Nat::BoundedDistance` (the arrow's left
    // side is uniformly the ranked subjects; the named view receives them as
    // arguments in order). This fires for every machine carrying the retired
    // shape, terminating or not, so the old spelling cannot linger inertly.
    for machine in program.machines() {
        if let Some(message) = retired_subtraction_message(program, machine) {
            diagnostics.push(Diagnostic::error(message));
        }
    }

    for machine in program.machines().iter().filter(|machine| {
        // TPR3 slice 1: the checker gates on the NORMALIZED plan (decision
        // 23) -- a machine claims termination when it authored the public
        // guarantee or supplied a ranking witness. The `terminates`
        // compatibility bool is populated from the same authorship and
        // agrees by construction until TPR6 retires it.
        let plan = &machine.termination_plan;
        matches!(
            &plan.interface,
            psi_language_semantics::TerminationInterface::Published(
                psi_language_semantics::TerminationGuarantee::Terminates { .. }
            )
        ) || plan.implementation_witness.is_some()
    }) {
        // A retired-spelling machine already has its directed diagnostic; the
        // ranking checks below would only stack a misleading "cannot prove".
        if retired_subtraction_message(program, machine).is_some() {
            continue;
        }

        if !graph::machine_has_cycle(program, machine) {
            continue;
        }

        if machine.termination_plan.implementation_witness.is_none() {
            diagnostics.push(Diagnostic::error(format!(
                "terminating machine {} contains a recursive cycle but carries no \
                 ranking witness -- spell one with `terminates by <subjects> [-> View];` \
                 (decision 23)",
                machine_name(program, machine.symbol)
            )));
            continue;
        }

        match ranking::machine_decrease_outcome(program, machine) {
            ranking::DecreaseOutcome::Proven => {}
            ranking::DecreaseOutcome::Unproven => {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove the `terminates by` ranking for machine {}",
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
            ranking::DecreaseOutcome::Rejected(message) => {
                diagnostics.push(Diagnostic::error(format!(
                    "terminating machine {}: {message}",
                    machine_name(program, machine.symbol)
                )));
            }
            ranking::DecreaseOutcome::PlanViewDivergence { recorded, resolved } => {
                // Internal invariant (TPR3 slice 1): the lowering's recorded
                // elaboration mirrors the checker's inference exactly; a
                // divergence means one changed without the other. Loud and
                // named, never silent.
                diagnostics.push(Diagnostic::error(format!(
                    "internal: terminating machine {}'s recorded ranking view `{recorded}` \
                     diverges from the checker's resolved view `{resolved}` (decision 23 \
                     firewall) -- the syntax->resolved elaboration and the checker's \
                     inference must stay in lockstep",
                    machine_name(program, machine.symbol)
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

/// TPR3 slice 4 (decision 23): build the checked termination facts -- the
/// `checked_summary`'s producer. Derives from the SAME pure resolution and
/// proof functions the check uses, so facts and diagnostics cannot
/// disagree; an unproven claimant records `NoGuarantee` AND fails
/// compilation, so a compiled artifact never carries an unestablished
/// claim. Every ACYCLIC checked body derives eventual termination without a
/// witness (the brief: "an acyclic checked body derives termination without
/// source annotation"). This local summary never publishes a promise: the
/// contract plan continues to read only `termination_plan.interface`.
pub(crate) fn build_termination_facts(
    program: &psi_typed_trees::TypedTrees,
) -> psi_checked_trees::TerminationFacts {
    use psi_language_semantics::TerminationGuarantee;

    let mut machines = Vec::new();
    for machine in program.machines() {
        let established = if retired_subtraction_message(program, machine).is_some() {
            false
        } else if !graph::machine_has_cycle(program, machine) {
            true
        } else if machine.termination_plan.implementation_witness.is_some() {
            matches!(
                ranking::machine_decrease_outcome(program, machine),
                ranking::DecreaseOutcome::Proven
            )
        } else {
            false
        };
        machines.push(psi_checked_trees::MachineTerminationFact {
            machine: machine.symbol,
            checked_summary: if established {
                TerminationGuarantee::Terminates {
                    premises: Vec::new(),
                }
            } else {
                TerminationGuarantee::NoGuarantee
            },
            resolved_view_path: ranking::machine_resolved_view_path(program, machine),
        });
    }
    psi_checked_trees::TerminationFacts { machines }
}

/// Render the diagnostic for a plain `terminates by value` clause whose value has
/// no inferable builtin ranking: name the value, say why inference failed, and
/// suggest the explicit `-> View` form. Declared measures matching the value's
/// type are suggested by name but are never selected implicitly — even a single
/// declared measure requires the explicit form, so declaring a second measure
/// later cannot silently change distant `terminates by` clauses.
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
            "select one with `terminates by {clause} -> View` \
             (builtin views: Nat::Descending, Nat::BoundedDistance, Slice::Length)"
        ),
        [only] => format!(
            "declared measures are never selected implicitly; \
             select one with `terminates by {clause} -> {only}`"
        ),
        many => format!(
            "declared measures are never selected implicitly; \
             select one with `terminates by {clause} -> View` (declared measures: {})",
            many.join(", ")
        ),
    };
    format!(
        "cannot infer a ranking for `terminates by {clause}` in machine {machine}: \
         {reason} -- {suggestion}"
    )
}

/// Render the diagnostic for a two-subject `terminates by` tuple whose subjects
/// are inverted: the swapped subjects prove as the named bounded distance, so
/// the message names the ranking and the corrected spelling instead of a bare
/// "cannot prove".
fn inverted_distance_message(machine: &str, inverted: &ranking::InvertedDistance) -> String {
    let declared = inverted.declared.as_str();
    let corrected = inverted.corrected.as_str();
    format!(
        "cannot prove the `terminates by` ranking for machine {machine}: \
         `terminates by {declared}` inverts the named bounded distance -- \
         `Nat::BoundedDistance` ranks `(lower, upper)`, which descends as the \
         lower value climbs; write `terminates by {corrected} -> Nat::BoundedDistance`"
    )
}

/// The retirement diagnostic for the use-site subtraction spelling, or `None`
/// when the witness subject is not a single top-level subtraction.
/// The message spells the exact argumented replacement, with the subtraction's
/// operands reordered into the view's `(lower, upper)` parameter order.
fn retired_subtraction_message(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> Option<String> {
    let [subject] = machine
        .termination_plan
        .implementation_witness
        .as_ref()?
        .subjects
        .as_slice()
    else {
        return None;
    };
    let (upper, lower) = subject.split_once(" - ")?;
    Some(format!(
        "the use-site subtraction `terminates by {upper} - {lower}` on machine {} is retired: \
         spell the ranking as `terminates by ({lower}, {upper}) -> Nat::BoundedDistance` \
         (the tuple lists the ranked subjects, bound in order to the view's \
         (lower, upper) parameters)",
        machine_name(program, machine.symbol)
    ))
}
