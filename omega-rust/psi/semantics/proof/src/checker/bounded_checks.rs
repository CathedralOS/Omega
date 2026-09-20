//! Bounded-value obligation checks for assignments, initializers, state
//! returns, call arguments and transition arguments.

use crate::checker::AssignmentRangeContext;
use crate::checker::assignment_stability::{
    collect_read_place_paths, expression_contains_call, member_paths_may_alias, written_place_path,
};
use crate::checker::certificate::{
    CertificateVerdict, bounded_integer_value_verdict, guarded_transition_integer_verdict_measured,
    state_return_integer_verdict,
};
use crate::checker::dependent_bounds::{
    dependent_call_field_floor, dependent_field_floor, guard_proves_dependent_upper,
    guard_proves_sibling_len_upper, sibling_len_from_constraints, state_preserves_field,
    symbolic_max_from_constraints,
};
use crate::checker::derivation_cache::DerivationConsultation;
use crate::checker::diagnostics::{
    cannot_prove_bounded_assignment_float, cannot_prove_bounded_assignment_integer,
    cannot_prove_bounded_call_float, cannot_prove_bounded_call_integer,
    cannot_prove_bounded_initializer_float, cannot_prove_bounded_initializer_integer,
    cannot_prove_bounded_return_float, cannot_prove_bounded_return_integer,
    cannot_prove_bounded_transition_float, cannot_prove_bounded_transition_integer,
    cannot_prove_dependent_call_bound, cannot_prove_dependent_transition_bound,
    expression_display_name,
};
use crate::checker::float_ranges::{
    float_range_for_assignment, float_range_for_call_argument, float_range_for_initializer,
    float_range_for_return_value, float_range_for_transition_argument,
    float_range_from_constraints,
};
use crate::checker::integer_ranges::{
    guarded_integer_range_for_assignment, guarded_integer_range_for_transition_argument,
    integer_range_for_call_argument, integer_range_for_initializer, integer_range_from_constraints,
    type_constraints,
};
use crate::checker::measurement::ProofPlanMeasurements;
use crate::checker::named_constraints::{
    check_assignment_named_constraints, check_call_named_constraints,
    check_initializer_named_constraints, check_return_named_constraints,
    check_transition_named_constraints,
};
use crate::checker::return_arrival;
use crate::obligations::{
    BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
    BoundedStateReturnObligation, BoundedTransitionArgumentObligation, ProofPlan,
};
use diagnostics::Diagnostic;
use numerics::bignum::BigInt;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

pub(crate) fn check_bounded_assignment(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    seed: u64,
    diagnostics: &mut Vec<Diagnostic>,
    measurements: &mut ProofPlanMeasurements,
    derivations: Option<&mut DerivationConsultation<'_>>,
) {
    // Chapter 11 invariant windows: an intermediate store need not itself
    // satisfy the place's constraints when a later store repairs the EXACT
    // place before anything can observe it.  The repairing assignment keeps
    // its own ordinary obligation; this only suppresses proof debt for a
    // value that is provably dead before the next consumption point.
    if assignment_is_overwritten_before_consumption(proof_plan, obligation) {
        return;
    }

    check_assignment_named_constraints(proof_plan, obligation, diagnostics);

    if let Some(target_range) =
        integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        // The certificate route decides the covered shapes on independently
        // checked evidence; everything else keeps the ordinary derivation.
        match bounded_integer_value_verdict(
            proof_plan,
            obligation.value,
            obligation.value_constraints,
            obligation.base_type,
            &target_range,
            seed,
            false,
            measurements,
            derivations,
        ) {
            CertificateVerdict::Certified => {}
            CertificateVerdict::Rejected => {
                diagnostics.push(cannot_prove_bounded_assignment_integer(
                    proof_plan,
                    obligation,
                    target_range,
                ));
            }
            CertificateVerdict::Uncovered => {
                let Some(value_range) =
                    guarded_integer_range_for_assignment(proof_plan, obligation)
                else {
                    diagnostics.push(cannot_prove_bounded_assignment_integer(
                        proof_plan,
                        obligation,
                        target_range,
                    ));
                    return;
                };

                if value_range.minimum < target_range.minimum
                    || value_range.maximum > target_range.maximum
                {
                    diagnostics.push(cannot_prove_bounded_assignment_integer(
                        proof_plan,
                        obligation,
                        target_range,
                    ));
                }
            }
        }
    }

    if let Some(target_range) =
        float_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(value_range) = float_range_for_assignment(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_assignment_float(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if !target_range.contains_range(&value_range) {
            diagnostics.push(cannot_prove_bounded_assignment_float(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }
}

/// Whether this assignment's value is overwritten before it can be observed.
///
/// Calls and transitions are unconditional consumption points. Reads of the
/// place and writes that may alias it also close the window. Pure work over
/// disjoint places may occur between the opening write and its repair.
fn assignment_is_overwritten_before_consumption(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
) -> bool {
    let program = proof_plan.program;
    let Some(state) = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine).iter())
        .find(|state| state.symbol == obligation.state_symbol)
    else {
        return false;
    };
    let Some(window_path) = written_place_path(proof_plan, obligation.target) else {
        return false;
    };
    // Indexed places intentionally stay strict for now: the path helper
    // collapses an element to its collection, so equal paths would not prove
    // that two dynamic indexes denote the same location.
    if matches!(
        program.expression_table.expression(obligation.target),
        ExpressionNode::Indexed(_)
    ) {
        return false;
    }
    let target_name = program.expression_table.display_name(obligation.target);
    let mut reached_assignment = false;

    for statement in program.statement_table.statements(state.statement_nodes) {
        if !reached_assignment {
            if let StatementNode::Assignment(assignment) = statement
                && assignment.target == obligation.target
                && assignment.value == obligation.value
            {
                reached_assignment = true;
            }
            continue;
        }

        match statement {
            // A proof assertion is a semantic consumption point: an open
            // invariant window may not flow through it.
            StatementNode::RootBinding(_) | StatementNode::AssemblyFact(_) => return false,
            StatementNode::Assignment(assignment) => {
                if expression_contains_call(proof_plan, assignment.value) {
                    return false;
                }
                let mut reads = Vec::new();
                collect_read_place_paths(proof_plan, assignment.value, &mut reads);
                if reads
                    .iter()
                    .any(|read| member_paths_may_alias(read, &window_path))
                {
                    return false;
                }

                let Some(written) = written_place_path(proof_plan, assignment.target) else {
                    return false;
                };
                if program.expression_table.display_name(assignment.target) == target_name {
                    return true;
                }
                if member_paths_may_alias(&written, &window_path) {
                    return false;
                }
            }
            StatementNode::LocalData(local) => {
                if local.initial_value.is_valid() {
                    if expression_contains_call(proof_plan, local.initial_value) {
                        return false;
                    }
                    let mut reads = Vec::new();
                    collect_read_place_paths(proof_plan, local.initial_value, &mut reads);
                    if reads
                        .iter()
                        .any(|read| member_paths_may_alias(read, &window_path))
                    {
                        return false;
                    }
                }
            }
            StatementNode::Expression(expression) => {
                if expression_contains_call(proof_plan, *expression) {
                    return false;
                }
                let mut reads = Vec::new();
                collect_read_place_paths(proof_plan, *expression, &mut reads);
                if reads
                    .iter()
                    .any(|read| member_paths_may_alias(read, &window_path))
                {
                    return false;
                }
            }
            StatementNode::Call(_) | StatementNode::Transition(_) => return false,
        }
    }

    false
}

pub(crate) fn check_bounded_initializer(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    seed: u64,
    diagnostics: &mut Vec<Diagnostic>,
    measurements: &mut ProofPlanMeasurements,
    derivations: Option<&mut DerivationConsultation<'_>>,
) {
    check_initializer_named_constraints(proof_plan, obligation, diagnostics);

    if let Some(target_range) =
        integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        // The certificate route covers the literal leg; initializers carry no
        // value constraints, so every other shape stays uncovered.
        match bounded_integer_value_verdict(
            proof_plan,
            obligation.value,
            arena::HandleSpan::empty(),
            obligation.base_type,
            &target_range,
            seed,
            false,
            measurements,
            derivations,
        ) {
            CertificateVerdict::Certified => {}
            CertificateVerdict::Rejected => {
                diagnostics.push(cannot_prove_bounded_initializer_integer(
                    proof_plan,
                    obligation,
                    target_range,
                ));
            }
            CertificateVerdict::Uncovered => {
                let Some(value_range) = integer_range_for_initializer(proof_plan, obligation)
                else {
                    diagnostics.push(cannot_prove_bounded_initializer_integer(
                        proof_plan,
                        obligation,
                        target_range,
                    ));
                    return;
                };

                if value_range.minimum < target_range.minimum
                    || value_range.maximum > target_range.maximum
                {
                    diagnostics.push(cannot_prove_bounded_initializer_integer(
                        proof_plan,
                        obligation,
                        target_range,
                    ));
                }
            }
        }
    }

    if let Some(target_range) =
        float_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(value_range) = float_range_for_initializer(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_initializer_float(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if !target_range.contains_range(&value_range) {
            diagnostics.push(cannot_prove_bounded_initializer_float(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }
}

pub(crate) fn check_bounded_state_return(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    context: &AssignmentRangeContext<'_>,
    seed: u64,
    diagnostics: &mut Vec<Diagnostic>,
    measurements: &mut ProofPlanMeasurements,
    derivations: Option<&mut DerivationConsultation<'_>>,
) {
    check_return_named_constraints(proof_plan, obligation, diagnostics);

    if let Some(target_range) =
        integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        // The certificate route covers the declared-or-literal legs the
        // arrival machinery would reach anyway; arrival bounds, contract
        // refinement and malformed obligations stay uncovered.
        match state_return_integer_verdict(
            proof_plan,
            obligation,
            &target_range,
            seed,
            measurements,
            derivations,
        ) {
            CertificateVerdict::Certified => {}
            CertificateVerdict::Rejected => {
                diagnostics.push(cannot_prove_bounded_return_integer(
                    proof_plan,
                    obligation,
                    target_range,
                ));
            }
            CertificateVerdict::Uncovered => {
                let Some(value_range) =
                    return_arrival::integer_range(proof_plan, obligation, context)
                else {
                    diagnostics.push(cannot_prove_bounded_return_integer(
                        proof_plan,
                        obligation,
                        target_range,
                    ));
                    return;
                };

                if value_range.minimum < target_range.minimum
                    || value_range.maximum > target_range.maximum
                {
                    diagnostics.push(cannot_prove_bounded_return_integer(
                        proof_plan,
                        obligation,
                        target_range,
                    ));
                }
            }
        }
    }

    if let Some(target_range) =
        float_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(value_range) = float_range_for_return_value(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_return_float(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if !target_range.contains_range(&value_range) {
            diagnostics.push(cannot_prove_bounded_return_float(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }
}

pub(crate) fn check_bounded_call_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    seed: u64,
    diagnostics: &mut Vec<Diagnostic>,
    measurements: &mut ProofPlanMeasurements,
    derivations: Option<&mut DerivationConsultation<'_>>,
) {
    check_call_named_constraints(proof_plan, obligation, diagnostics);

    if let Some(target_range) =
        integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        // The certificate route covers the anonymous, literal and declared
        // legs in the same order the ordinary derivation tries them.
        match bounded_integer_value_verdict(
            proof_plan,
            obligation.argument,
            obligation.argument_constraints,
            obligation.base_type,
            &target_range,
            seed,
            true,
            measurements,
            derivations,
        ) {
            CertificateVerdict::Certified => {}
            CertificateVerdict::Rejected => {
                diagnostics.push(cannot_prove_bounded_call_integer(
                    proof_plan,
                    obligation,
                    target_range,
                ));
            }
            CertificateVerdict::Uncovered => {
                let Some(argument_range) = integer_range_for_call_argument(proof_plan, obligation)
                else {
                    diagnostics.push(cannot_prove_bounded_call_integer(
                        proof_plan,
                        obligation,
                        target_range,
                    ));
                    return;
                };

                if argument_range.minimum < target_range.minimum
                    || argument_range.maximum > target_range.maximum
                {
                    diagnostics.push(cannot_prove_bounded_call_integer(
                        proof_plan,
                        obligation,
                        target_range,
                    ));
                }
            }
        }
    }

    // R1 dependent maximum on a CALL argument: no co-located guard exists on
    // a call statement, so the only rung-A discharge is the worst case
    // through the field's OWN enforced minimum -- and only for SELF-receiver
    // calls (the recognizer's `self.<field>` names the callee's data, which
    // for a self-call IS this machine's; cross-machine dependent params are
    // the R4 boundary-witness rung). Anything else refuses loudly.
    if let Some((minimum, max_field, max_offset)) =
        symbolic_max_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let self_receiver = obligation
            .receiver
            .as_ref()
            .is_none_or(|receiver| receiver.as_str() == "self");
        let argument_range = integer_range_for_call_argument(proof_plan, obligation);
        // Route (c), as on transitions: a same-field tighter-or-equal
        // dependent argument forwards (self-receiver keeps `self.<field>`
        // the same place).
        let argument_atom = symbolic_max_from_constraints(type_constraints(
            proof_plan,
            obligation.argument_constraints,
        ));
        let atom_proves = argument_atom.is_some_and(|(arg_min, arg_field, arg_offset)| {
            arg_field.as_str() == max_field.as_str()
                && arg_offset <= max_offset
                && arg_min >= minimum
        }) && state_preserves_field(
            proof_plan,
            obligation.machine.as_str(),
            obligation.state.as_str(),
            max_field,
        );
        let proven = self_receiver
            && (atom_proves
                || argument_range.is_some_and(|range| {
                    range.minimum >= BigInt::from_i64(minimum)
                        && dependent_call_field_floor(proof_plan, obligation, max_field)
                            .and_then(|floor| floor.checked_add(max_offset))
                            .is_some_and(|cap| range.maximum <= BigInt::from_i64(cap))
                }));
        if !proven {
            diagnostics.push(cannot_prove_dependent_call_bound(
                proof_plan, obligation, minimum, max_field, max_offset,
            ));
        }
    }

    // Sibling-length params on CALL arguments: a call has no co-located
    // guard and a slice length has no static floor, so no rung-A discharge
    // exists -- refuse with the route hint.
    if let Some((_, max_offset)) =
        sibling_len_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let _ = max_offset;
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove call argument `{}` satisfies sibling-length parameter `{}` for `{}` in `{}.{}`; route the call through a transition whose arm guards `{} < <items>.len`",
            expression_display_name(proof_plan, obligation.argument),
            obligation.parameter,
            obligation.target,
            obligation.machine,
            obligation.state,
            expression_display_name(proof_plan, obligation.argument),
        )));
    }

    if let Some(target_range) =
        float_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(argument_range) = float_range_for_call_argument(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_call_float(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if !target_range.contains_range(&argument_range) {
            diagnostics.push(cannot_prove_bounded_call_float(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }
}

pub(crate) fn check_bounded_transition_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    seed: u64,
    diagnostics: &mut Vec<Diagnostic>,
    measurements: &mut ProofPlanMeasurements,
    mut derivations: Option<&mut DerivationConsultation<'_>>,
) {
    check_transition_named_constraints(proof_plan, obligation, diagnostics);

    if let Some(target_range) =
        integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        // The certificate route covers the anonymous, literal and declared
        // legs in the same order the ordinary derivation tries them, then the
        // guard-narrowed legs (direct and `place +- K` refold); point
        // exclusions and the arrival rescue stay on the uncovered path.
        match bounded_integer_value_verdict(
            proof_plan,
            obligation.argument,
            obligation.argument_constraints,
            obligation.base_type,
            &target_range,
            seed,
            true,
            measurements,
            derivations.as_deref_mut(),
        ) {
            CertificateVerdict::Certified => {}
            CertificateVerdict::Rejected => {
                diagnostics.push(cannot_prove_bounded_transition_integer(
                    proof_plan,
                    obligation,
                    target_range,
                ));
            }
            CertificateVerdict::Uncovered => {
                // The guard-narrowed leg cites the arm's guard and refuted
                // exit guards as explicit premises on the argument's atom;
                // shapes it cannot certify keep the trusted narrowing below.
                match guarded_transition_integer_verdict_measured(
                    proof_plan,
                    obligation,
                    &target_range,
                    seed,
                    measurements,
                    derivations,
                ) {
                    CertificateVerdict::Certified => {}
                    CertificateVerdict::Rejected => {
                        diagnostics.push(cannot_prove_bounded_transition_integer(
                            proof_plan,
                            obligation,
                            target_range,
                        ));
                    }
                    CertificateVerdict::Uncovered => {
                        let argument_range =
                            guarded_integer_range_for_transition_argument(proof_plan, obligation);

                        if argument_range.minimum < target_range.minimum
                            || argument_range.maximum > target_range.maximum
                        {
                            // Use the shared arithmetic owner for this exact occurrence.
                            // Named/dependent constraints remain independent checks below.
                            let arrival_fits = validation::arrival_integer_expression_bounds(
                                proof_plan.program,
                                obligation.machine_symbol,
                                obligation.state_symbol,
                                obligation.statement_index,
                                obligation.argument,
                            )
                            .is_some_and(|(minimum, maximum)| {
                                BigInt::from_i64(minimum) >= target_range.minimum
                                    && BigInt::from_i64(maximum) <= target_range.maximum
                            });
                            if !arrival_fits {
                                diagnostics.push(cannot_prove_bounded_transition_integer(
                                    proof_plan,
                                    obligation,
                                    target_range,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // R1 dependent maximum (`i: u32 [0..=self.count]`): the upper half is a
    // RELATIONAL obligation -- `arg <= self.count + offset` at the transition
    // point. Discharge routes, in order: (a) the arm's own guard relates the
    // argument to the SAME field (`arg < self.count`; co-located, so no
    // stability gate -- same rationale as the literal path above); (b) worst
    // case through the field's OWN enforced literal range (`arg_max <=
    // min(count) + offset` holds for every runtime count). The lower half is
    // the literal minimum, checked against the guarded argument range.
    if let Some((minimum, max_field, max_offset)) =
        symbolic_max_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let argument_range = guarded_integer_range_for_transition_argument(proof_plan, obligation);
        // Route (c): the ARGUMENT is itself dependent-ranged on the SAME
        // field with a tighter-or-equal bound (`i: [0..=self.count]`
        // forwarded into `k: [0..=self.count]`; the exclusive sugar's -1
        // offset forwards into the inclusive form). Transitions stay within
        // one machine, so the field is the same place.
        let argument_atom = symbolic_max_from_constraints(type_constraints(
            proof_plan,
            obligation.argument_constraints,
        ));
        let atom_proves = argument_atom.is_some_and(|(arg_min, arg_field, arg_offset)| {
            arg_field.as_str() == max_field.as_str()
                && arg_offset <= max_offset
                && arg_min >= minimum
        }) && state_preserves_field(
            proof_plan,
            obligation.machine.as_str(),
            obligation.state.as_str(),
            max_field,
        );
        let lower_proven = atom_proves || argument_range.minimum >= BigInt::from_i64(minimum);
        let upper_proven = atom_proves
            || guard_proves_dependent_upper(proof_plan, obligation, max_field, max_offset)
            || dependent_field_floor(proof_plan, obligation, max_field)
                .and_then(|floor| floor.checked_add(max_offset))
                .is_some_and(|cap| argument_range.maximum <= BigInt::from_i64(cap));
        if !lower_proven || !upper_proven {
            diagnostics.push(cannot_prove_dependent_transition_bound(
                proof_plan, obligation, minimum, max_field, max_offset,
            ));
        }
    }

    // R1 sibling-length maximum (`index: u64 [0..items.len]`): the argument
    // must sit under the SIBLING ARGUMENT's length at this call -- and slice
    // lengths have no static floor, so the only discharge is the co-located
    // guard relating the argument to `<sibling-arg>.len` tightly enough.
    if let Some((minimum, max_offset)) =
        sibling_len_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let argument_range = guarded_integer_range_for_transition_argument(proof_plan, obligation);
        let lower_proven = argument_range.minimum >= BigInt::from_i64(minimum);
        let upper_proven = obligation.sibling_argument.is_valid()
            && guard_proves_sibling_len_upper(
                proof_plan,
                obligation.argument,
                &obligation.guard,
                obligation.sibling_argument,
                max_offset,
            );
        if !lower_proven || !upper_proven {
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove transition argument `{}` satisfies sibling-length parameter `{}` in `{}.{}`; expected {minimum}..=<sibling>.len{} -- relate them on the arm (`{} < {}.len`)",
                expression_display_name(proof_plan, obligation.argument),
                obligation.parameter,
                obligation.machine,
                obligation.state,
                if max_offset == 0 { String::new() } else { format!(" {:+}", max_offset) },
                expression_display_name(proof_plan, obligation.argument),
                if obligation.sibling_argument.is_valid() {
                    expression_display_name(proof_plan, obligation.sibling_argument)
                } else {
                    "<sibling>".to_string()
                },
            )));
        }
    }

    if let Some(target_range) =
        float_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(argument_range) = float_range_for_transition_argument(proof_plan, obligation)
        else {
            diagnostics.push(cannot_prove_bounded_transition_float(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if !target_range.contains_range(&argument_range) {
            diagnostics.push(cannot_prove_bounded_transition_float(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }
}
