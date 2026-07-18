mod graph;
mod machine_calls;
mod order;
mod ranking;

use crate::labels::machine_name;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::expression::{BinaryOperator, ExpressionNode};

/// Normalize a trait requirement's published completion guarantee onto its
/// concrete satisfier. The implementation does not need to repeat
/// `terminates;`; its own `terminates by ...;` remains private evidence.
pub(crate) fn inherit_requirement_guarantees(program: &mut omega_typed_trees::TypedTrees) {
    let inherited: Vec<usize> = program
        .machines()
        .iter()
        .enumerate()
        .filter_map(|(index, machine)| {
            if machine.termination_guarantee.is_eventual_terminal() {
                return None;
            }
            let entry_name = program
                .machine_states(machine)
                .first()
                .map(|state| state.name.as_str());
            let inherits = program
                .machine_trait_conformances(machine)
                .iter()
                .any(|conformance| {
                    let Some(trait_definition) = program
                        .traits()
                        .iter()
                        .find(|candidate| candidate.symbol == conformance.symbol)
                    else {
                        return false;
                    };
                    let requirement_name = conformance
                        .requirement
                        .as_ref()
                        .map(|name| name.as_str())
                        .or_else(|| {
                            if machine.attached_data.is_none() {
                                Some(machine.name.as_str())
                            } else {
                                entry_name
                            }
                        });
                    program
                        .trait_machine_signatures(trait_definition)
                        .iter()
                        .any(|requirement| {
                            requirement_name == Some(requirement.name.as_str())
                                && requirement.termination_guarantee.is_eventual_terminal()
                        })
                });
            inherits.then_some(index)
        })
        .collect();

    for index in inherited {
        program.machines_mut()[index].termination_guarantee =
            omega_core::termination::TerminationGuarantee::EventualTerminal;
    }
}

/// Elaborate short-form witnesses to their stable builtin canonical view.
///
/// This pass runs before validation/checking and mutates only the private
/// witness. It never selects a declared user measure, even when exactly one
/// matches, so declaration-set changes cannot reinterpret source or affect a
/// published contract identity.
pub(crate) fn elaborate_canonical_ranking_views(program: &mut omega_typed_trees::TypedTrees) {
    let mut elaborations: Vec<(usize, &'static [&'static str])> = Vec::new();

    for (index, machine) in program.machines().iter().enumerate() {
        if !machine.ranking_witness.is_present() || !machine.ranking_witness.view.is_empty() {
            continue;
        }
        let Some(root_state) = program.machine_states(machine).first() else {
            continue;
        };
        let subjects = program
            .expression_table
            .expression_handles(machine.ranking_witness.subjects);
        let view = match order::RankingOrder::resolve(program, root_state, subjects, &[], &[]) {
            order::OrderResolution::Resolved(order::RankingOrder::NatDescending) => {
                &["Nat", "Descending"][..]
            }
            order::OrderResolution::Resolved(order::RankingOrder::SliceLength) => {
                &["Slice", "Length"][..]
            }
            _ => continue,
        };
        elaborations.push((index, view));
    }

    for (index, members) in elaborations {
        let mut view = omega_core::arena::HandleSpan::empty();
        for member in members {
            program.signature_effects.append_to_span(
                &mut view,
                omega_typed_trees::name::Identifier::generated(*member),
            );
        }
        program.machines_mut()[index].ranking_witness.view = view;
    }
}

pub(crate) fn check_machine_termination(
    program: &omega_typed_trees::TypedTrees,
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
        machine.termination_guarantee.is_eventual_terminal() || machine.ranking_witness.is_present()
    }) {
        // A retired-spelling machine already has its directed diagnostic; the
        // ranking checks below would only stack a misleading "cannot prove".
        if retired_subtraction_message(program, machine).is_some() {
            continue;
        }

        if !graph::machine_has_cycle(program, machine) {
            continue;
        }

        if !machine.ranking_witness.is_present() {
            diagnostics.push(Diagnostic::error(format!(
                "machine {} publishes termination for a recursive cycle but has no `terminates by` ranking witness",
                machine_name(program, machine.symbol)
            )));
            continue;
        }

        match ranking::machine_decrease_outcome(program, machine) {
            ranking::DecreaseOutcome::Proven => {}
            ranking::DecreaseOutcome::Unproven => {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove `terminates by` ranking witness for machine {}",
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
            ranking::DecreaseOutcome::UnprovenRange(range) => {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove rank range `{range}` for `terminates by` witness in machine {}",
                    machine_name(program, machine.symbol)
                )));
            }
        }
    }

    diagnostics.extend(machine_calls::check_joint_machine_call_cycles(program));

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Build the local checked completion summary after the termination gate has
/// accepted the program. Acyclic bodies derive completion without source
/// annotation; a proven cyclic witness derives it privately. Neither case
/// changes the published machine interface.
pub(crate) fn checked_termination_summaries(
    program: &omega_typed_trees::TypedTrees,
) -> Vec<omega_checked_trees::MachineTerminationSummary> {
    program
        .machines()
        .iter()
        .map(|machine| {
            let guarantee = if !graph::machine_has_cycle(program, machine)
                || machine.ranking_witness.is_present()
                    && matches!(
                        ranking::machine_decrease_outcome(program, machine),
                        ranking::DecreaseOutcome::Proven
                    )
            {
                omega_core::termination::TerminationGuarantee::EventualTerminal
            } else {
                omega_core::termination::TerminationGuarantee::None
            };
            omega_checked_trees::MachineTerminationSummary {
                machine: machine.symbol,
                guarantee,
            }
        })
        .collect()
}

/// Render the diagnostic for a short-form `terminates by value` witness whose value has
/// no inferable builtin ranking: name the value, say why inference failed, and
/// suggest the explicit `-> View` form. Declared measures matching the value's
/// type are suggested by name but are never selected implicitly — even a single
/// declared measure requires the explicit form, so declaring a second measure
/// later cannot silently change distant ranking witnesses.
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
            "select one with `terminates by {clause} -> View;` \
             (builtin views: Nat::Descending, Nat::IncreasingTo(bound), \
             Nat::BoundedDistance, Slice::Length)"
        ),
        [only] => format!(
            "declared measures are never selected implicitly; \
             select one with `terminates by {clause} -> {only};`"
        ),
        many => format!(
            "declared measures are never selected implicitly; \
             select one with `terminates by {clause} -> View;` (declared measures: {})",
            many.join(", ")
        ),
    };
    format!(
        "cannot infer a ranking view for `terminates by {clause};` in machine {machine}: \
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
        "cannot prove `terminates by` ranking witness for machine {machine}: \
         `terminates by {declared}` inverts the named bounded distance -- \
         `Nat::BoundedDistance` ranks `(lower, upper)`, which descends as the \
         lower value climbs; write `terminates by {corrected} -> Nat::BoundedDistance;`"
    )
}

/// The retirement diagnostic for the use-site subtraction spelling, or `None`
/// when the machine's ranking witness is not a single top-level subtraction.
/// The message spells the exact argumented replacement, with the subtraction's
/// operands reordered into the view's `(lower, upper)` parameter order.
fn retired_subtraction_message(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
) -> Option<String> {
    let [decreases] = program
        .expression_table
        .expression_handles(machine.ranking_witness.subjects)
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
        "the use-site subtraction `terminates by {upper} - {lower};` on machine {} is retired: \
         spell the ranking as `terminates by ({lower}, {upper}) -> Nat::BoundedDistance;` \
         (the tuple lists the ranked subjects, bound in order to the view's \
         (lower, upper) parameters)",
        machine_name(program, machine.symbol)
    ))
}
