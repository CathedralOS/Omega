use crate::obligations::{
    BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
    BoundedStateReturnObligation, BoundedTransitionArgumentObligation, IntegerRange,
    ProofConstraint, ProofObligation, ProofPlan, dehoisted_condition, dehoisted_operand,
    integer_binary_range,
};
use arena::HandleSpan;
use diagnostics::Diagnostic;
use numerics::bignum::BigInt;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::name::Identifier;
use typed_trees::statement::{StatementNode, TransitionGuardNode};

mod arrival_stability;
mod return_arrival;

#[derive(Debug, Clone, Copy, PartialEq)]
struct FloatRange {
    minimum: f64,
    maximum: f64,
}

pub fn check_proof_plan(proof_plan: &ProofPlan) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let range_context = AssignmentRangeContext::new(proof_plan);

    for (_, obligation) in proof_plan.obligations.iter() {
        match obligation {
            ProofObligation::BoundedAssignment(obligation) => {
                check_bounded_assignment(proof_plan, obligation, &mut diagnostics);
            }
            ProofObligation::BoundedCallArgument(obligation) => {
                check_bounded_call_argument(proof_plan, obligation, &mut diagnostics);
            }
            ProofObligation::BoundedInitializer(obligation) => {
                check_bounded_initializer(proof_plan, obligation, &mut diagnostics);
            }
            ProofObligation::BoundedStateReturn(obligation) => {
                check_bounded_state_return(
                    proof_plan,
                    obligation,
                    &range_context,
                    &mut diagnostics,
                );
            }
            ProofObligation::BoundedTransitionArgument(obligation) => {
                check_bounded_transition_argument(proof_plan, obligation, &mut diagnostics);
            }
            ProofObligation::BoundedValue(_) | ProofObligation::GuardedTransition(_) => {}
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn check_bounded_assignment(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    diagnostics: &mut Vec<Diagnostic>,
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
        let Some(value_range) = guarded_integer_range_for_assignment(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_assignment_integer(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_assignment_integer(
                proof_plan,
                obligation,
                target_range,
            ));
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

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
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
            StatementNode::AssemblyFact(_) => return false,
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

fn check_bounded_initializer(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_initializer_named_constraints(proof_plan, obligation, diagnostics);

    if let Some(target_range) =
        integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(value_range) = integer_range_for_initializer(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_initializer_integer(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_initializer_integer(
                proof_plan,
                obligation,
                target_range,
            ));
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

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_initializer_float(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }
}

fn check_bounded_state_return(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    context: &AssignmentRangeContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_return_named_constraints(proof_plan, obligation, diagnostics);

    if let Some(target_range) =
        integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(value_range) = return_arrival::integer_range(proof_plan, obligation, context)
        else {
            diagnostics.push(cannot_prove_bounded_return_integer(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_return_integer(
                proof_plan,
                obligation,
                target_range,
            ));
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

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_return_float(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }
}

fn check_bounded_call_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_call_named_constraints(proof_plan, obligation, diagnostics);

    if let Some(target_range) =
        integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(argument_range) = integer_range_for_call_argument(proof_plan, obligation) else {
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

        if argument_range.minimum < target_range.minimum
            || argument_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_call_float(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }
}

fn check_bounded_transition_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_transition_named_constraints(proof_plan, obligation, diagnostics);

    if let Some(target_range) =
        integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let argument_range = guarded_integer_range_for_transition_argument(proof_plan, obligation);

        if argument_range.minimum < target_range.minimum
            || argument_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_transition_integer(
                proof_plan,
                obligation,
                target_range,
            ));
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

        if argument_range.minimum < target_range.minimum
            || argument_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_transition_float(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }
}

fn integer_range_for_transition_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
) -> Option<IntegerRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.argument)
    {
        ExpressionNode::Integer(value) => integer_range_for_literal(value),
        _ => integer_range_from_constraints(type_constraints(
            proof_plan,
            obligation.argument_constraints,
        )),
    }
}

fn guarded_integer_range_for_transition_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
) -> IntegerRange {
    let base =
        integer_range_for_transition_argument(proof_plan, obligation).unwrap_or_else(neutral_range);

    // Co-located: the arm's guard and its arguments evaluate at the SAME
    // dispatch, so the guard fact needs no stability gate here (collection
    // downgrades the guard when a sibling argument contains an opaque call).
    let range = apply_handle_guard(proof_plan, base, obligation.argument, &obligation.guard);
    let mut range =
        guard_refined_binary_range(proof_plan, range, obligation.argument, &obligation.guard);
    // Fall-through complements: control reaching this transition refuted
    // every prior exit guard (collection gates on call-free arguments), so
    // each applies with its comparison INVERTED -- directly on the argument
    // place, and through the `place +- K` refold.
    for refuted in &obligation.refuted_exit_guards {
        range = apply_handle_condition_complement(proof_plan, range, obligation.argument, *refuted);
        range = complement_refined_binary_range(proof_plan, range, obligation.argument, *refuted);
    }
    range
}

/// `apply_handle_condition` with the comparison REFUTED: `place == K` gives
/// point exclusion (bump an end sitting exactly on K), `place < K` gives
/// `place >= K`, and so on. Conjunctions cannot refute soundly (either leg
/// may have failed) and are skipped.
fn apply_handle_condition_complement(
    proof_plan: &ProofPlan,
    mut range: IntegerRange,
    argument: ExpressionHandle,
    condition: ExpressionHandle,
) -> IntegerRange {
    let condition = unwrap_true_guard_condition(proof_plan, condition);
    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(condition)
    else {
        return range;
    };
    if !expressions_equivalent_for_proof(proof_plan, binary.left, argument) {
        return range;
    }
    let Some(value) = integer_literal_handle(proof_plan, binary.right) else {
        return range;
    };
    let value = BigInt::from_i64(value);
    let one = BigInt::from_i64(1);
    match binary.operator {
        // NOT (place == K): exclude the point when an end sits on it.
        BinaryOperator::Equal => {
            if range.minimum == value {
                range.minimum = range.minimum.add(&one);
            }
            if range.maximum == value {
                range.maximum = range.maximum.sub(&one);
            }
        }
        // NOT (place < K)  ==  place >= K
        BinaryOperator::Less => range.minimum = range.minimum.max(value),
        // NOT (place <= K)  ==  place >= K + 1
        BinaryOperator::LessOrEqual => range.minimum = range.minimum.max(value.add(&one)),
        // NOT (place > K)  ==  place <= K
        BinaryOperator::Greater => range.maximum = range.maximum.min(value),
        // NOT (place >= K)  ==  place <= K - 1
        BinaryOperator::GreaterOrEqual => range.maximum = range.maximum.min(value.sub(&one)),
        _ => {}
    }
    range
}

/// `guard_refined_binary_range`'s complement twin: refine a `place +- K`
/// argument by narrowing the PLACE operand with a REFUTED prior guard and
/// refolding.
fn complement_refined_binary_range(
    proof_plan: &ProofPlan,
    range: IntegerRange,
    value: ExpressionHandle,
    refuted: ExpressionHandle,
) -> IntegerRange {
    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(value)
    else {
        return range;
    };
    let (place, literal, place_is_left) =
        if let Some(literal) = integer_literal_handle(proof_plan, binary.right) {
            (binary.left, literal, true)
        } else if let Some(literal) = integer_literal_handle(proof_plan, binary.left) {
            (binary.right, literal, false)
        } else {
            return range;
        };
    let literal = BigInt::from_i64(literal);
    let place_range = match (binary.operator, place_is_left) {
        (BinaryOperator::Add, _) => IntegerRange {
            minimum: range.minimum.sub(&literal),
            maximum: range.maximum.sub(&literal),
        },
        (BinaryOperator::Subtract, true) => IntegerRange {
            minimum: range.minimum.add(&literal),
            maximum: range.maximum.add(&literal),
        },
        (BinaryOperator::Subtract, false) => IntegerRange {
            minimum: literal.sub(&range.maximum),
            maximum: literal.sub(&range.minimum),
        },
        _ => return range,
    };
    let narrowed =
        apply_handle_condition_complement(proof_plan, place_range.clone(), place, refuted);
    if narrowed == place_range {
        return range;
    }
    let refolded = match (binary.operator, place_is_left) {
        (BinaryOperator::Add, _) => IntegerRange {
            minimum: narrowed.minimum.add(&literal),
            maximum: narrowed.maximum.add(&literal),
        },
        (BinaryOperator::Subtract, true) => IntegerRange {
            minimum: narrowed.minimum.sub(&literal),
            maximum: narrowed.maximum.sub(&literal),
        },
        (BinaryOperator::Subtract, false) => IntegerRange {
            minimum: literal.sub(&narrowed.maximum),
            maximum: literal.sub(&narrowed.minimum),
        },
        _ => unreachable!("classified above"),
    };
    IntegerRange {
        minimum: range.minimum.max(refolded.minimum),
        maximum: range.maximum.min(refolded.maximum),
    }
}

fn float_range_for_transition_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.argument)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange {
                minimum: value,
                maximum: value,
            })
        }
        _ => float_range_from_constraints(type_constraints(
            proof_plan,
            obligation.argument_constraints,
        )),
    }
}

fn float_range_for_assignment(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange {
                minimum: value,
                maximum: value,
            })
        }
        _ => {
            float_range_from_constraints(type_constraints(proof_plan, obligation.value_constraints))
        }
    }
}

fn float_range_for_call_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.argument)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange {
                minimum: value,
                maximum: value,
            })
        }
        _ => float_range_from_constraints(type_constraints(
            proof_plan,
            obligation.argument_constraints,
        )),
    }
}

fn float_range_for_return_value(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange {
                minimum: value,
                maximum: value,
            })
        }
        _ => {
            float_range_from_constraints(type_constraints(proof_plan, obligation.value_constraints))
        }
    }
}

fn float_range_for_initializer(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange {
                minimum: value,
                maximum: value,
            })
        }
        _ => None,
    }
}

fn integer_range_for_call_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
) -> Option<IntegerRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.argument)
    {
        ExpressionNode::Integer(value) => integer_range_for_literal(value),
        _ => integer_range_from_constraints(type_constraints(
            proof_plan,
            obligation.argument_constraints,
        )),
    }
}

fn integer_range_for_assignment(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
) -> Option<IntegerRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Integer(value) => integer_range_for_literal(value),
        _ => integer_range_from_constraints(type_constraints(
            proof_plan,
            obligation.value_constraints,
        )),
    }
}

fn guarded_integer_range_for_assignment(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
) -> Option<IntegerRange> {
    let context = AssignmentRangeContext::new(proof_plan);
    guarded_integer_range_for_assignment_with_context(proof_plan, obligation, &context)
}

fn guarded_integer_range_for_assignment_with_context(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    context: &AssignmentRangeContext<'_>,
) -> Option<IntegerRange> {
    // An UNRANGED integer value starts NEUTRAL (the full i64 line) instead of
    // bailing, so a stable edge guard ALONE can establish its range -- the
    // guarded-COPY shape `transition self.yv >= 0 && self.yv <= 9 { true ->
    // store() }` then `self.y = self.yv` used to return None here before the
    // guard was ever consulted. Starting wider is sound: guard refinement only
    // intersects, and a bound the guard leaves at the i64 extreme fails the
    // target fit exactly as the old None did.
    let declared = integer_range_for_assignment(proof_plan, obligation);
    let mut range = declared.clone().unwrap_or_else(neutral_range);

    // R4 containment intake: boundary-ensures witnesses live at this
    // assignment clamp the value's range -- directly when the VALUE is the
    // witnessed place, and through the binary refold below when an OPERAND
    // is (the witness fills the upper end the declaration leaves open; the
    // place's type floor supplies the lower).
    let value_display = proof_plan
        .program
        .expression_table
        .display_name(obligation.value);
    for (place, bound) in &obligation.ensures_witness_bounds {
        if place == &value_display {
            let bound = BigInt::from_i64(*bound);
            if range.maximum > bound {
                range.maximum = bound;
            }
        }
    }
    // Witness-only binary refold: the witness carries its own stability
    // (computed at build with the invalidation walk), so it needs no
    // incoming state guard -- `self.m = self.n + 1` after `ensures size <=
    // 8` refolds n's [0, 8] through the addition with no guard at all.
    if !obligation.ensures_witness_bounds.is_empty()
        && let Some(operands) = &obligation.binary_operands
    {
        let witness_operand = |declared: &Option<IntegerRange>, handle: ExpressionHandle| {
            let mut narrowed = declared.clone().unwrap_or_else(neutral_range);
            let operand_display = proof_plan.program.expression_table.display_name(handle);
            let mut touched = false;
            for (place, bound) in &obligation.ensures_witness_bounds {
                if place == &operand_display {
                    let bound = BigInt::from_i64(*bound);
                    if narrowed.maximum > bound {
                        narrowed.maximum = bound;
                    }
                    if narrowed.minimum < BigInt::zero()
                        && operand_is_unsigned(proof_plan, obligation, handle)
                    {
                        narrowed.minimum = BigInt::zero();
                    }
                    touched = true;
                }
            }
            (touched || declared.is_some()).then_some(narrowed)
        };
        if let (Some(left), Some(right)) = (
            witness_operand(&operands.left_range, operands.left),
            witness_operand(&operands.right_range, operands.right),
        ) && left != neutral_range()
            && right != neutral_range()
            && let Some(folded) = integer_binary_range(operands.operator, left, right)
        {
            range = IntegerRange {
                minimum: range.minimum.max(folded.minimum),
                maximum: range.maximum.min(folded.maximum),
            };
        }
    }

    // The incoming-edge guard held at STATE ENTRY; it still holds at this
    // assignment only if nothing earlier in the state could have changed what
    // it constrained (a prior write to a may-aliasing place, or any opaque
    // call). Without this gate, `transition c < 100 { true -> bump() }` with
    // `bump { c = 100; c = c + 1 }` would "prove" the second write.
    if let Some(guard) = &obligation.state_guard
        && assignment_guard_is_stable(proof_plan, obligation, guard, context)
    {
        range = apply_assignment_guard(proof_plan, range, obligation.value, guard);
        range = guard_refined_binary_range(proof_plan, range, obligation.value, guard);

        // OPERAND-wise refold of a top-level binary value: each operand's
        // range = its DECLARED range (resolved at build time), with the guard
        // filling in one the declaration leaves unbounded -- `self.p +
        // self.dir` with `p: [0..=8]` declared and `dir` bounded only by the
        // incoming `dir >= 0 && dir <= 1`. The whole-value fold dies at build
        // time on the unranged operand, and `guard_refined_binary_range`
        // above is place-vs-LITERAL only, so neither reaches this shape.
        if let TransitionGuardNode::When(condition) = guard
            && let Some(operands) = &obligation.binary_operands
        {
            let operand_range = |declared: Option<IntegerRange>, handle: ExpressionHandle| {
                let base = declared.unwrap_or_else(neutral_range);
                let mut narrowed = apply_source_condition(
                    proof_plan,
                    base,
                    handle,
                    *condition,
                    obligation.machine_symbol,
                    obligation.state_guard_source,
                );
                // R4: an ensures-witnessed OPERAND place clamps here; an
                // unsigned place's type floor supplies the lower end.
                let operand_display = proof_plan.program.expression_table.display_name(handle);
                for (place, bound) in &obligation.ensures_witness_bounds {
                    if place == &operand_display {
                        let bound = BigInt::from_i64(*bound);
                        if narrowed.maximum > bound {
                            narrowed.maximum = bound;
                        }
                        if narrowed.minimum < BigInt::zero()
                            && operand_is_unsigned(proof_plan, obligation, handle)
                        {
                            narrowed.minimum = BigInt::zero();
                        }
                    }
                }
                (narrowed != neutral_range()).then_some(narrowed)
            };
            if let (Some(left), Some(right)) = (
                operand_range(operands.left_range.clone(), operands.left),
                operand_range(operands.right_range.clone(), operands.right),
            ) && let Some(folded) = integer_binary_range(operands.operator, left, right)
            {
                range = IntegerRange {
                    minimum: range.minimum.max(folded.minimum),
                    maximum: range.maximum.min(folded.maximum),
                };
            }
        }
    }

    // Nothing declared AND nothing narrowed: keep reporting "no range" rather
    // than a vacuous full-line interval.
    if declared.is_none() && range == neutral_range() {
        return None;
    }
    Some(range)
}

/// Invocation-local custody for assignment-range queries over one immutable
/// proof plan. Reusing it changes no frame result: the resolver's cache keys
/// bind exact call nodes and owning machines from the same typed program.
pub struct AssignmentRangeContext<'program> {
    program: &'program typed_trees::TypedTrees,
    call_frames: std::sync::OnceLock<Option<validation::CallFrameResolver<'program>>>,
}

impl<'program> AssignmentRangeContext<'program> {
    pub fn new(proof_plan: &ProofPlan<'program>) -> Self {
        Self {
            program: proof_plan.program,
            call_frames: std::sync::OnceLock::new(),
        }
    }

    fn call_frames(&self) -> Option<&validation::CallFrameResolver<'program>> {
        self.call_frames
            .get_or_init(|| validation::CallFrameResolver::new(self.program))
            .as_ref()
    }
}

/// The integer range Psi proves for one assignment value after applying its
/// declared constraints, stable incoming guard, and retained boundary witness
/// facts. The proof plan carries every assignment site, not only sites whose
/// semantic destination is itself constrained. Returning `None` means Psi has
/// no bounded fact to publish; later lowering must remain fail-closed.
pub fn proved_assignment_integer_range(
    proof_plan: &ProofPlan<'_>,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
) -> Option<crate::obligations::IntegerRange> {
    let obligation =
        assignment_range_obligation(proof_plan, machine_symbol, state_symbol, statement_index)?;
    guarded_integer_range_for_assignment(proof_plan, obligation)
}

pub fn proved_assignment_integer_range_with_context(
    proof_plan: &ProofPlan<'_>,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    context: &AssignmentRangeContext<'_>,
) -> Option<crate::obligations::IntegerRange> {
    if !std::ptr::eq(context.program, proof_plan.program) {
        return None;
    }
    let obligation =
        assignment_range_obligation(proof_plan, machine_symbol, state_symbol, statement_index)?;
    guarded_integer_range_for_assignment_with_context(proof_plan, obligation, context)
}

fn assignment_range_obligation<'plan>(
    proof_plan: &'plan ProofPlan<'_>,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
) -> Option<&'plan BoundedAssignmentObligation> {
    proof_plan
        .assignment_value_ranges
        .iter()
        .map(|(_, obligation)| obligation)
        .find(|obligation| {
            obligation.machine_symbol == machine_symbol
                && obligation.state_symbol == state_symbol
                && obligation.statement_index == statement_index
        })
}

/// Whether an operand place's DECLARED primitive is unsigned (its type
/// floor is 0) -- lets an ensures upper witness pair with the natural
/// lower bound.
fn operand_is_unsigned(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    handle: ExpressionHandle,
) -> bool {
    let _ = obligation;
    let Some(constraints) = operand_declared_primitive(proof_plan, handle) else {
        return false;
    };
    matches!(
        constraints,
        typed_trees::types::PrimitiveType::U8
            | typed_trees::types::PrimitiveType::U16
            | typed_trees::types::PrimitiveType::U32
            | typed_trees::types::PrimitiveType::U64
            | typed_trees::types::PrimitiveType::Addr
    )
}

fn operand_declared_primitive(
    proof_plan: &ProofPlan,
    handle: ExpressionHandle,
) -> Option<typed_trees::types::PrimitiveType> {
    // Member place (`self.n`): resolve through the attached data's field.
    let program = proof_plan.program;
    let ExpressionNode::Member(member) = program.expression_table.expression(handle) else {
        return None;
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver) else {
        return None;
    };
    let [receiver] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if receiver.as_str() != "self" {
        return None;
    }
    for machine in program.machines() {
        let Some(attached) = machine.attached_data.as_ref() else {
            continue;
        };
        let Some(data) = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == attached.as_str())
        else {
            continue;
        };
        if let Some(field_type) =
            crate::obligations::data_field_type_by_name(program, data, member.member.as_str())
        {
            return program.primitive_type_reference(field_type);
        }
    }
    None
}

/// Whether the incoming-edge guard's facts survive from state entry to THIS
/// assignment: every earlier statement in the state must have a complete write
/// frame provably DISJOINT from every place the guard condition or assignment
/// value reads. Prefix member paths alias (`self.state` vs
/// `self.state.count`); distinct roots do not (`self.pixels[i]` vs `self.i` --
/// the render-loop shape stays provable). Resolved pure calls therefore
/// preserve the guard; opaque frames and unsupported statement shapes still
/// drop it (sound).
fn assignment_guard_is_stable(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    guard: &TransitionGuardNode,
    context: &AssignmentRangeContext<'_>,
) -> bool {
    use typed_trees::statement::StatementNode;

    let program = proof_plan.program;
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == obligation.machine_symbol)
    else {
        return false;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == obligation.state_symbol)
    else {
        return false;
    };

    // Every place the guard fact (and the value it refines) depends on. The
    // DE-HOISTED binary operands are included too: the obligation value may
    // spell `__hoist_N + 1` while the guard fact is applied to the hoisted
    // read's PLACE (`tallies[self.k]`), so an aliasing write into the
    // collection (or to the index) must drop the fact even though the value's
    // own read path is only the local name.
    // GUARD reads and VALUE reads are tracked separately: a body `let` that
    // DEFINES a name the value reads (the operand hoist's own
    // `let __hoist_N = tallies[self.k]`) is the fact's SOURCE, not an
    // invalidation -- but a `let` shadowing a name the GUARD read (the guard
    // evaluates in the SOURCE state's scope, so a same-named body local is a
    // different binding) must still kill the fact, and any ASSIGNMENT
    // aliasing either list still kills it.
    let mut guard_read_paths: Vec<Vec<String>> = Vec::new();
    if let TransitionGuardNode::When(condition) = guard {
        collect_read_place_paths(proof_plan, *condition, &mut guard_read_paths);
    }
    let mut read_paths: Vec<Vec<String>> = guard_read_paths.clone();
    collect_read_place_paths(proof_plan, obligation.value, &mut read_paths);
    if let Some(operands) = &obligation.binary_operands {
        collect_read_place_paths(proof_plan, operands.left, &mut read_paths);
        collect_read_place_paths(proof_plan, operands.right, &mut read_paths);
    }
    let Some(call_frames) = context.call_frames() else {
        return false;
    };

    let statements = program.statement_table.statements(state.statement_nodes);
    if !matches!(statements.get(obligation.statement_index),
        Some(StatementNode::Assignment(assignment))
            if assignment.target == obligation.target && assignment.value == obligation.value)
    {
        return false;
    }
    arrival_stability::prefix_preserves_reads(
        proof_plan,
        machine,
        state,
        obligation.statement_index,
        &guard_read_paths,
        &read_paths,
        call_frames,
    )
}

fn resolved_writes_overlap_reads(written: &[String], reads: &[Vec<String>]) -> bool {
    reads.iter().any(|read| {
        let read = read.join(".");
        written
            .iter()
            .any(|write| validation::frame_paths_overlap(&read, write))
    })
}

/// The member path a place expression READS or WRITES, for the aliasing check:
/// `self.state.count` -> [self, state, count]; an INDEXED place resolves to its
/// collection's path (a write anywhere inside the collection aliases the whole
/// collection, nothing else). `None` for shapes the walk cannot name (treated
/// as opaque by callers).
fn written_place_path(proof_plan: &ProofPlan, expression: ExpressionHandle) -> Option<Vec<String>> {
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => written_place_path(proof_plan, inner.target),
        ExpressionNode::Indexed(indexed) => written_place_path(proof_plan, indexed.collection),
        ExpressionNode::Member(member) => {
            let mut path = written_place_path(proof_plan, member.receiver)?;
            path.push(member.member.as_str().to_owned());
            Some(path)
        }
        ExpressionNode::Name(path) => Some(
            proof_plan
                .program
                .expression_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str().to_owned())
                .collect(),
        ),
        _ => None,
    }
}

/// Collect the member paths of every Name/Member read inside `expression`
/// (guard conditions, assignment values). Unnameable reads (indexed elements)
/// contribute their COLLECTION path, so a write into the collection kills the
/// fact.
fn collect_read_place_paths(
    proof_plan: &ProofPlan,
    expression: ExpressionHandle,
    paths: &mut Vec<Vec<String>>,
) {
    if !expression.is_valid() {
        return;
    }
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            collect_read_place_paths(proof_plan, binary.left, paths);
            collect_read_place_paths(proof_plan, binary.right, paths);
        }
        ExpressionNode::Unary(unary) => {
            collect_read_place_paths(proof_plan, unary.operand, paths);
        }
        ExpressionNode::Cast(cast) => collect_read_place_paths(proof_plan, cast.value, paths),
        ExpressionNode::Borrow(inner) => collect_read_place_paths(proof_plan, inner.target, paths),
        ExpressionNode::Indexed(indexed) => {
            collect_read_place_paths(proof_plan, indexed.collection, paths);
            collect_read_place_paths(proof_plan, indexed.index, paths);
        }
        ExpressionNode::Member(_) | ExpressionNode::Name(_) => {
            if let Some(path) = written_place_path(proof_plan, expression) {
                paths.push(path);
            }
        }
        _ => {}
    }
}

/// Two member paths may alias when one is a PREFIX of the other (a whole-struct
/// write aliases every field under it, and vice versa).
fn member_paths_may_alias(left: &[String], right: &[String]) -> bool {
    let shared = left.len().min(right.len());
    left[..shared] == right[..shared]
}

/// Whether any `Call` node appears in the expression tree (an opaque effect:
/// a value-machine call may mutate fields through `&mut self`).
fn expression_contains_call(proof_plan: &ProofPlan, expression: ExpressionHandle) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Call(_) => true,
        ExpressionNode::Binary(binary) => {
            expression_contains_call(proof_plan, binary.left)
                || expression_contains_call(proof_plan, binary.right)
        }
        ExpressionNode::Unary(unary) => expression_contains_call(proof_plan, unary.operand),
        ExpressionNode::Cast(cast) => expression_contains_call(proof_plan, cast.value),
        ExpressionNode::Borrow(inner) => expression_contains_call(proof_plan, inner.target),
        ExpressionNode::Indexed(indexed) => {
            expression_contains_call(proof_plan, indexed.collection)
                || expression_contains_call(proof_plan, indexed.index)
        }
        ExpressionNode::Member(member) => expression_contains_call(proof_plan, member.receiver),
        _ => false,
    }
}

/// The dominating-guard KEYSTONE: refine the folded range of a
/// `<place> + K` / `<place> - K` / `K - <place>` value by narrowing the PLACE
/// operand with the guard and refolding. `range` soundly bounds the value, so
/// the place's implied bound INVERTS from it algebraically; the guard
/// tightens it (via `apply_handle_condition`, which matches the place
/// structurally, understands `&&`, and reads either literal side); the refold
/// intersects back into `range`. This is what lets a state entered through
/// `c < 100` prove `c = c + 1` into a `[0..=100]` target -- the guard-proven
/// counter -- instead of forcing a Trapping/Wrapping domain or the modular
/// idiom.
fn guard_refined_binary_range(
    proof_plan: &ProofPlan,
    range: IntegerRange,
    value: ExpressionHandle,
    guard: &TransitionGuardNode,
) -> IntegerRange {
    let TransitionGuardNode::When(condition) = guard else {
        return range;
    };
    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(value)
    else {
        return range;
    };
    let (place, literal, place_is_left) =
        if let Some(literal) = integer_literal_handle(proof_plan, binary.right) {
            (binary.left, literal, true)
        } else if let Some(literal) = integer_literal_handle(proof_plan, binary.left) {
            (binary.right, literal, false)
        } else {
            return range;
        };
    // value = place + K  =>  place = value - K (and the subtract mirrors).
    let literal = BigInt::from_i64(literal);
    let place_range = match (binary.operator, place_is_left) {
        (BinaryOperator::Add, _) => IntegerRange {
            minimum: range.minimum.sub(&literal),
            maximum: range.maximum.sub(&literal),
        },
        (BinaryOperator::Subtract, true) => IntegerRange {
            minimum: range.minimum.add(&literal),
            maximum: range.maximum.add(&literal),
        },
        (BinaryOperator::Subtract, false) => IntegerRange {
            minimum: literal.sub(&range.maximum),
            maximum: literal.sub(&range.minimum),
        },
        _ => return range,
    };
    let narrowed = apply_handle_condition(proof_plan, place_range.clone(), place, *condition);
    if narrowed == place_range {
        return range;
    }
    let refolded = match (binary.operator, place_is_left) {
        (BinaryOperator::Add, _) => IntegerRange {
            minimum: narrowed.minimum.add(&literal),
            maximum: narrowed.maximum.add(&literal),
        },
        (BinaryOperator::Subtract, true) => IntegerRange {
            minimum: narrowed.minimum.sub(&literal),
            maximum: narrowed.maximum.sub(&literal),
        },
        (BinaryOperator::Subtract, false) => IntegerRange {
            minimum: literal.sub(&narrowed.maximum),
            maximum: literal.sub(&narrowed.minimum),
        },
        _ => unreachable!("classified above"),
    };
    IntegerRange {
        minimum: range.minimum.max(refolded.minimum),
        maximum: range.maximum.min(refolded.maximum),
    }
}

fn integer_range_for_return_value(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
) -> Option<IntegerRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Integer(value) => integer_range_for_literal(value),
        _ => integer_range_from_constraints(type_constraints(
            proof_plan,
            obligation.value_constraints,
        )),
    }
}

fn integer_range_for_initializer(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
) -> Option<IntegerRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Integer(value) => integer_range_for_literal(value),
        _ => None,
    }
}

/// The "know nothing" starting interval for guard refinement: the i64 line.
/// Sound as a start (guard refinement only intersects, and an end the guard
/// leaves at the extreme fails any spellable target fit); NOT a claim about
/// the value.
fn neutral_range() -> IntegerRange {
    IntegerRange {
        minimum: BigInt::from_i64(i64::MIN),
        maximum: BigInt::from_i64(i64::MAX),
    }
}

/// The `[v, v]` interval for a literal -- exact at any magnitude (N2); the
/// D14 width gate still owns which POSITIONS may spell an oversize literal.
fn integer_range_for_literal(literal: &numerics::literals::IntegerLiteral) -> Option<IntegerRange> {
    let value = literal.value_bignum()?;
    Some(IntegerRange {
        minimum: value.clone(),
        maximum: value,
    })
}

fn integer_range_from_constraints(constraints: &[ProofConstraint]) -> Option<IntegerRange> {
    let mut range: Option<IntegerRange> = None;

    for constraint in constraints {
        let ProofConstraint::IntegerRange { minimum, maximum } = constraint else {
            continue;
        };

        let candidate = IntegerRange {
            minimum: minimum.clone(),
            maximum: maximum.clone(),
        };

        range = Some(match range {
            Some(existing) => IntegerRange {
                minimum: existing.minimum.max(candidate.minimum),
                maximum: existing.maximum.min(candidate.maximum),
            },
            None => candidate,
        });
    }

    // Named sign facts RAISE an existing floor only (see the obligations-
    // side twin: the old standalone [0, i64::MAX] was a false upper claim
    // for u64 atoms).
    for constraint in constraints {
        let ProofConstraint::Named(name) = constraint else {
            continue;
        };
        let floor = match name.as_str() {
            "non_negative" => BigInt::zero(),
            "positive" => BigInt::from_i64(1),
            _ => continue,
        };
        if let Some(existing) = range.as_mut()
            && existing.minimum < floor
        {
            existing.minimum = floor;
        }
    }

    range
}

fn type_constraints<'proof>(
    proof_plan: &'proof ProofPlan<'_>,
    constraints: HandleSpan<ProofConstraint>,
) -> &'proof [ProofConstraint] {
    proof_plan.type_constraints.span(constraints).unwrap_or(&[])
}

fn float_range_from_constraints(constraints: &[ProofConstraint]) -> Option<FloatRange> {
    let mut range: Option<FloatRange> = None;

    for constraint in constraints {
        let ProofConstraint::FloatRange { minimum, maximum } = constraint else {
            continue;
        };

        let candidate = FloatRange {
            minimum: minimum.value(),
            maximum: maximum.value(),
        };

        range = Some(match range {
            Some(existing) => FloatRange {
                minimum: existing.minimum.max(candidate.minimum),
                maximum: existing.maximum.min(candidate.maximum),
            },
            None => candidate,
        });
    }

    range
}

fn finite_float_literal(value: &typed_trees::expression::FloatLiteral) -> Option<f64> {
    let value = value.value();
    value.is_finite().then_some(value)
}

fn integer_literal_handle(proof_plan: &ProofPlan, expression: ExpressionHandle) -> Option<i64> {
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.value_i64(),
        ExpressionNode::Name(path)
            if proof_plan
                .program
                .expression_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str())
                .eq(["u32", "MAX"]) =>
        {
            Some(u32::MAX as i64)
        }
        _ => None,
    }
}

fn apply_handle_guard(
    proof_plan: &ProofPlan,
    range: IntegerRange,
    argument: ExpressionHandle,
    guard: &TransitionGuardNode,
) -> IntegerRange {
    match guard {
        TransitionGuardNode::Always => range,
        TransitionGuardNode::When(condition) => {
            apply_handle_condition(proof_plan, range, argument, *condition)
        }
    }
}

fn apply_assignment_guard(
    proof_plan: &ProofPlan,
    range: IntegerRange,
    value: ExpressionHandle,
    guard: &TransitionGuardNode,
) -> IntegerRange {
    let range = apply_handle_guard(proof_plan, range, value, guard);

    let ExpressionNode::Binary(value_binary) =
        proof_plan.program.expression_table.expression(value)
    else {
        return range;
    };
    if value_binary.operator != BinaryOperator::Subtract {
        return range;
    }

    let TransitionGuardNode::When(condition) = guard else {
        return range;
    };
    let condition = unwrap_true_guard_condition(proof_plan, *condition);
    let ExpressionNode::Binary(condition_binary) =
        proof_plan.program.expression_table.expression(condition)
    else {
        return range;
    };

    let lower_bound =
        if expressions_equivalent_for_proof(proof_plan, value_binary.left, condition_binary.left)
            && expressions_equivalent_for_proof(
                proof_plan,
                value_binary.right,
                condition_binary.right,
            )
        {
            match condition_binary.operator {
                BinaryOperator::Greater => Some(1),
                BinaryOperator::GreaterOrEqual => Some(0),
                _ => None,
            }
        } else if expressions_equivalent_for_proof(
            proof_plan,
            value_binary.left,
            condition_binary.right,
        ) && expressions_equivalent_for_proof(
            proof_plan,
            value_binary.right,
            condition_binary.left,
        ) {
            match condition_binary.operator {
                BinaryOperator::Less => Some(1),
                BinaryOperator::LessOrEqual => Some(0),
                _ => None,
            }
        } else {
            None
        };

    let Some(lower_bound) = lower_bound else {
        return range;
    };

    IntegerRange {
        minimum: range.minimum.max(BigInt::from_i64(lower_bound)),
        maximum: range.maximum,
    }
}

fn unwrap_true_guard_condition(
    proof_plan: &ProofPlan,
    condition: ExpressionHandle,
) -> ExpressionHandle {
    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(condition)
    else {
        return condition;
    };

    if binary.operator == BinaryOperator::Equal {
        if matches!(
            proof_plan.program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(true)
        ) {
            return binary.left;
        }

        if matches!(
            proof_plan.program.expression_table.expression(binary.left),
            ExpressionNode::Boolean(true)
        ) {
            return binary.right;
        }
    }

    condition
}

fn apply_handle_condition(
    proof_plan: &ProofPlan,
    range: IntegerRange,
    argument: ExpressionHandle,
    condition: ExpressionHandle,
) -> IntegerRange {
    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(condition)
    else {
        return range;
    };

    if binary.operator == BinaryOperator::Equal {
        if matches!(
            proof_plan.program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(true)
        ) {
            return apply_handle_condition(proof_plan, range, argument, binary.left);
        }

        if matches!(
            proof_plan.program.expression_table.expression(binary.left),
            ExpressionNode::Boolean(true)
        ) {
            return apply_handle_condition(proof_plan, range, argument, binary.right);
        }
    }

    if binary.operator == BinaryOperator::And {
        let range = apply_handle_condition(proof_plan, range, argument, binary.left);
        return apply_handle_condition(proof_plan, range, argument, binary.right);
    }

    if expressions_equivalent_for_proof(proof_plan, binary.left, argument) {
        return apply_right_literal_guard(proof_plan, range, binary.operator, binary.right);
    }

    if expressions_equivalent_for_proof(proof_plan, binary.right, argument) {
        return apply_left_literal_guard(proof_plan, range, binary.left, binary.operator);
    }

    range
}

/// `apply_handle_condition` with GUARD-SIDE de-hoisting: the guard's compared
/// sides may spell a hoisted local from the SOURCE state's scope (an indexed
/// guard subject is frontend-hoisted to `let __hoist_N = tallies[self.k]`), so
/// each side is resolved through that state's call-free place initializers
/// before the structural match. Only used where the obligation carries the
/// guard's source state; co-located guards keep the plain applier.
fn apply_source_condition(
    proof_plan: &ProofPlan,
    range: IntegerRange,
    argument: ExpressionHandle,
    condition: ExpressionHandle,
    machine_symbol: symbols::SymbolHandle,
    source_state: symbols::SymbolHandle,
) -> IntegerRange {
    // A hoisted GUARD SUBJECT is a bare name whose initializer is the actual
    // comparison (`transition __hoist_N { .. }`): resolve it in the SOURCE
    // state's scope before matching.
    let condition = proof_plan
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .and_then(|machine| {
            proof_plan
                .program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == source_state)
                .map(|state| dehoisted_condition(proof_plan.program, state, condition))
        })
        .unwrap_or(condition);
    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(condition)
    else {
        return range;
    };

    if binary.operator == BinaryOperator::Equal {
        if matches!(
            proof_plan.program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(true)
        ) {
            return apply_source_condition(
                proof_plan,
                range,
                argument,
                binary.left,
                machine_symbol,
                source_state,
            );
        }
        if matches!(
            proof_plan.program.expression_table.expression(binary.left),
            ExpressionNode::Boolean(true)
        ) {
            return apply_source_condition(
                proof_plan,
                range,
                argument,
                binary.right,
                machine_symbol,
                source_state,
            );
        }
    }

    if binary.operator == BinaryOperator::And {
        let range = apply_source_condition(
            proof_plan,
            range,
            argument,
            binary.left,
            machine_symbol,
            source_state,
        );
        return apply_source_condition(
            proof_plan,
            range,
            argument,
            binary.right,
            machine_symbol,
            source_state,
        );
    }

    let dehoist = |handle: ExpressionHandle| {
        let program = proof_plan.program;
        program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
            .and_then(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .find(|state| state.symbol == source_state)
                    .map(|state| dehoisted_operand(program, state, handle))
            })
            .unwrap_or(handle)
    };

    let left = dehoist(binary.left);
    let right = dehoist(binary.right);
    if expressions_equivalent_for_proof(proof_plan, left, argument) {
        return apply_right_literal_guard(proof_plan, range, binary.operator, right);
    }
    if expressions_equivalent_for_proof(proof_plan, right, argument) {
        return apply_left_literal_guard(proof_plan, range, left, binary.operator);
    }

    range
}

fn expressions_equivalent_for_proof(
    proof_plan: &ProofPlan,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    if left == right {
        return true;
    }

    // The SAME place is spelled two ways across statements: a flat Name path
    // (`Name(["self", "k"])`) or a Member chain (`Member(Name(["self"]), "k")`).
    // Flatten both to segment lists and compare -- without this, a guard's
    // `self.k` never matched a hoisted initializer's `self.k` and the fact
    // silently failed to apply. `flat_place_segments` returns None for
    // anything indexed/called, which falls through to the structural arms.
    if let (Some(left_path), Some(right_path)) = (
        flat_place_segments(proof_plan, left),
        flat_place_segments(proof_plan, right),
    ) {
        return left_path == right_path;
    }

    match (
        proof_plan.program.expression_table.expression(left),
        proof_plan.program.expression_table.expression(right),
    ) {
        (ExpressionNode::Borrow(left), _) => {
            expressions_equivalent_for_proof(proof_plan, left.target, right)
        }
        (_, ExpressionNode::Borrow(right)) => {
            expressions_equivalent_for_proof(proof_plan, left, right.target)
        }
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            proof_plan
                .program
                .expression_table
                .name_path_members(left.members)
                == proof_plan
                    .program
                    .expression_table
                    .name_path_members(right.members)
        }
        (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
            left.target == right.target
                && left.target_symbol == right.target_symbol
                && left.arguments.count() == right.arguments.count()
                && match (left.receiver.is_valid(), right.receiver.is_valid()) {
                    (true, true) => {
                        expressions_equivalent_for_proof(proof_plan, left.receiver, right.receiver)
                    }
                    (false, false) => true,
                    _ => false,
                }
                && proof_plan
                    .program
                    .expression_table
                    .expression_handles(left.arguments)
                    .iter()
                    .zip(
                        proof_plan
                            .program
                            .expression_table
                            .expression_handles(right.arguments),
                    )
                    .all(|(left_argument, right_argument)| {
                        expressions_equivalent_for_proof(
                            proof_plan,
                            *left_argument,
                            *right_argument,
                        )
                    })
        }
        (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
            left.member == right.member
                && left.member_symbol == right.member_symbol
                && expressions_equivalent_for_proof(proof_plan, left.receiver, right.receiver)
        }
        // An INDEXED place (`self.tallies[1]`, `self.tallies[self.k]`): same
        // collection, same index. Lets a guard fact on an element (`tallies[1]
        // < 16`) refine the element's read in the guarded state -- the
        // accumulate-into-array keystone (`tallies[1] = tallies[1] + 1`).
        // Sound under the stability gate: the guard's read paths include the
        // COLLECTION (any indexed write into it aliases) and the INDEX
        // variable (a write to `k` drops the fact).
        (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
            let (left, right) = (*left, *right);
            expressions_equivalent_for_proof(proof_plan, left.collection, right.collection)
                && expressions_equivalent_for_proof(proof_plan, left.index, right.index)
        }
        // LEAVES: identical guards/places from different statements hold
        // distinct handles, so literal sub-terms (a `[1]` index, a compared
        // constant) must compare by VALUE (same fix as the precondition twin).
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
        (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
        (ExpressionNode::String(left), ExpressionNode::String(right)) => left == right,
        _ => false,
    }
}

/// A pure member place flattened to its name segments (`self.k` ->
/// ["self", "k"]), through `Mutable`. `None` for anything indexed, called, or
/// non-place -- those compare structurally.
fn flat_place_segments(
    proof_plan: &ProofPlan,
    expression: ExpressionHandle,
) -> Option<Vec<String>> {
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => flat_place_segments(proof_plan, inner.target),
        ExpressionNode::Name(path) => Some(
            proof_plan
                .program
                .expression_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str().to_owned())
                .collect(),
        ),
        ExpressionNode::Member(member) => {
            let mut segments = flat_place_segments(proof_plan, member.receiver)?;
            segments.push(member.member.as_str().to_owned());
            Some(segments)
        }
        _ => None,
    }
}

fn apply_right_literal_guard(
    proof_plan: &ProofPlan,
    mut range: IntegerRange,
    operator: BinaryOperator,
    right: ExpressionHandle,
) -> IntegerRange {
    let Some(value) = integer_literal_handle(proof_plan, right) else {
        return range;
    };

    let value = BigInt::from_i64(value);
    let one = BigInt::from_i64(1);
    match operator {
        BinaryOperator::Equal => {
            range.minimum = range.minimum.max(value.clone());
            range.maximum = range.maximum.min(value);
        }
        BinaryOperator::Greater => range.minimum = range.minimum.max(value.add(&one)),
        BinaryOperator::GreaterOrEqual => range.minimum = range.minimum.max(value),
        BinaryOperator::Less => range.maximum = range.maximum.min(value.sub(&one)),
        BinaryOperator::LessOrEqual => range.maximum = range.maximum.min(value),
        BinaryOperator::Add
        | BinaryOperator::And
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Divide
        | BinaryOperator::Modulo
        | BinaryOperator::Multiply
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Subtract => {}
    }

    range
}

fn apply_left_literal_guard(
    proof_plan: &ProofPlan,
    mut range: IntegerRange,
    left: ExpressionHandle,
    operator: BinaryOperator,
) -> IntegerRange {
    let Some(value) = integer_literal_handle(proof_plan, left) else {
        return range;
    };

    let value = BigInt::from_i64(value);
    let one = BigInt::from_i64(1);
    match operator {
        BinaryOperator::Equal => {
            range.minimum = range.minimum.max(value.clone());
            range.maximum = range.maximum.min(value);
        }
        BinaryOperator::Greater => range.maximum = range.maximum.min(value.sub(&one)),
        BinaryOperator::GreaterOrEqual => range.maximum = range.maximum.min(value),
        BinaryOperator::Less => range.minimum = range.minimum.max(value.add(&one)),
        BinaryOperator::LessOrEqual => range.minimum = range.minimum.max(value),
        BinaryOperator::Add
        | BinaryOperator::And
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Divide
        | BinaryOperator::Modulo
        | BinaryOperator::Multiply
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Subtract => {}
    }

    range
}

fn check_call_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !argument_handle_satisfies_named_constraint(
            proof_plan,
            obligation.argument,
            obligation.argument_constraints,
            constraint,
        ) {
            diagnostics.push(cannot_prove_call_named_constraint(
                proof_plan, obligation, constraint,
            ));
        }
    }
}

fn check_assignment_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !assignment_satisfies_named_constraint(proof_plan, obligation, constraint) {
            diagnostics.push(cannot_prove_assignment_named_constraint(
                proof_plan, obligation, constraint,
            ));
        }
    }
}

fn check_transition_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !transition_argument_satisfies_named_constraint(proof_plan, obligation, constraint) {
            diagnostics.push(cannot_prove_transition_named_constraint(
                proof_plan, obligation, constraint,
            ));
        }
    }
}

fn check_return_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !argument_handle_satisfies_named_constraint(
            proof_plan,
            obligation.value,
            obligation.value_constraints,
            constraint,
        ) {
            diagnostics.push(cannot_prove_return_named_constraint(
                proof_plan, obligation, constraint,
            ));
        }
    }
}

fn check_initializer_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !initializer_satisfies_named_constraint(proof_plan, obligation, constraint) {
            diagnostics.push(cannot_prove_initializer_named_constraint(
                proof_plan, obligation, constraint,
            ));
        }
    }
}

fn named_constraints(constraints: &[ProofConstraint]) -> impl Iterator<Item = &str> {
    constraints.iter().filter_map(|constraint| {
        let ProofConstraint::Named(name) = constraint else {
            return None;
        };

        Some(name.as_str())
    })
}

fn argument_handle_satisfies_named_constraint(
    proof_plan: &ProofPlan,
    argument: ExpressionHandle,
    argument_constraints: HandleSpan<ProofConstraint>,
    constraint: &str,
) -> bool {
    let constraints = type_constraints(proof_plan, argument_constraints);

    constraints_satisfy_named_constraint(constraints, constraint)
        || match (
            constraint,
            proof_plan.program.expression_table.expression(argument),
        ) {
            ("exact", ExpressionNode::Integer(_)) => true,
            ("finite", ExpressionNode::Float(value)) => finite_float_literal(value).is_some(),
            ("finite", ExpressionNode::Integer(_)) => true,
            ("non_negative", ExpressionNode::Integer(value)) => {
                value.value_i64().is_some_and(|value| value >= 0)
            }
            ("positive", ExpressionNode::Integer(value)) => {
                value.value_i64().is_some_and(|value| value > 0)
            }
            ("wrapping", ExpressionNode::Integer(_)) => true,
            _ => false,
        }
}

fn transition_argument_satisfies_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    constraint: &str,
) -> bool {
    let constraints = type_constraints(proof_plan, obligation.argument_constraints);

    if constraints_satisfy_named_constraint(constraints, constraint) {
        return true;
    }

    if matches!(constraint, "positive" | "non_negative") {
        let range = guarded_integer_range_for_transition_argument(proof_plan, obligation);
        return match constraint {
            "positive" => !range.minimum.is_negative() && !range.minimum.is_zero(),
            "non_negative" => !range.minimum.is_negative(),
            _ => false,
        };
    }

    if matches!(constraint, "exact")
        && integer_range_for_transition_argument(proof_plan, obligation)
            .is_some_and(|range| range.minimum == range.maximum)
    {
        return true;
    }

    argument_handle_satisfies_named_constraint(
        proof_plan,
        obligation.argument,
        obligation.argument_constraints,
        constraint,
    )
}

fn assignment_satisfies_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    constraint: &str,
) -> bool {
    let constraints = type_constraints(proof_plan, obligation.value_constraints);

    if constraints_satisfy_named_constraint(constraints, constraint) {
        return true;
    }

    if matches!(constraint, "positive" | "non_negative")
        && let Some(range) = guarded_integer_range_for_assignment(proof_plan, obligation)
    {
        return match constraint {
            "positive" => !range.minimum.is_negative() && !range.minimum.is_zero(),
            "non_negative" => !range.minimum.is_negative(),
            _ => false,
        };
    }

    if matches!(constraint, "exact")
        && guarded_integer_range_for_assignment(proof_plan, obligation)
            .is_some_and(|range| range.minimum == range.maximum)
    {
        return true;
    }

    argument_handle_satisfies_named_constraint(
        proof_plan,
        obligation.value,
        obligation.value_constraints,
        constraint,
    )
}

fn initializer_satisfies_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    constraint: &str,
) -> bool {
    match (
        constraint,
        proof_plan
            .program
            .expression_table
            .expression(obligation.value),
    ) {
        ("finite", ExpressionNode::Float(value)) => finite_float_literal(value).is_some(),
        ("exact", ExpressionNode::Integer(_)) => true,
        ("non_negative", ExpressionNode::Integer(value)) => {
            value.value_i64().is_some_and(|value| value >= 0)
        }
        ("positive", ExpressionNode::Integer(value)) => {
            value.value_i64().is_some_and(|value| value > 0)
        }
        ("wrapping", ExpressionNode::Integer(_)) => true,
        _ => false,
    }
}

fn constraints_satisfy_named_constraint(constraints: &[ProofConstraint], constraint: &str) -> bool {
    if constraints.iter().any(|argument_constraint| {
        matches!(
            argument_constraint,
            ProofConstraint::Named(argument_constraint)
                if argument_constraint.as_str() == constraint
        )
    }) {
        return true;
    }

    match constraint {
        "exact" => integer_range_from_constraints(constraints).is_some(),
        "finite" => {
            integer_range_from_constraints(constraints).is_some()
                || float_range_from_constraints(constraints).is_some()
        }
        "non_negative" => integer_range_from_constraints(constraints)
            .is_some_and(|range| !range.minimum.is_negative()),
        "positive" => integer_range_from_constraints(constraints)
            .is_some_and(|range| !range.minimum.is_negative() && !range.minimum.is_zero()),
        "wrapping" => false,
        _ => false,
    }
}

/// The single dependent-maximum atom of a constraint set (R1a mints at most
/// one: the declared range's own bound).
fn symbolic_max_from_constraints(
    constraints: &[ProofConstraint],
) -> Option<(i64, &Identifier, i64)> {
    constraints.iter().find_map(|constraint| match constraint {
        ProofConstraint::IntegerRangeSymbolicMax {
            minimum,
            max_field,
            max_offset,
        } => Some((*minimum, max_field, *max_offset)),
        _ => None,
    })
}

/// Route (a): the arm's guard conjunction contains a compare relating the
/// ARGUMENT (matched by display spelling -- the co-located guard names the
/// same expression the argument position does) to `self.<max_field> + k`,
/// tight enough for the declared offset: `arg < f + k` gives `arg <= f+k-1`
/// (needs `k-1 <= offset`); `arg <= f + k` needs `k <= offset`. Flipped
/// spellings (`f + k > arg`) normalize to the same two forms.
fn guard_proves_dependent_upper(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    max_field: &Identifier,
    max_offset: i64,
) -> bool {
    let TransitionGuardNode::When(guard) = obligation.guard else {
        return false;
    };
    let argument_label = expression_display_name(proof_plan, obligation.argument);
    guard_conjunct_proves_dependent_upper(proof_plan, guard, &argument_label, max_field, max_offset)
}

fn guard_conjunct_proves_dependent_upper(
    proof_plan: &ProofPlan,
    guard: ExpressionHandle,
    argument_label: &str,
    max_field: &Identifier,
    max_offset: i64,
) -> bool {
    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(guard)
    else {
        return false;
    };
    let recurse = |handle: ExpressionHandle| {
        guard_conjunct_proves_dependent_upper(
            proof_plan,
            handle,
            argument_label,
            max_field,
            max_offset,
        )
    };
    // `arg REL bound` normalized to (argument side, bound side, inclusive?).
    let normalized = match binary.operator {
        BinaryOperator::And => return recurse(binary.left) || recurse(binary.right),
        // The multi-arm desugar nests the spelled compare inside
        // `(subject) == true` (same shape the D14 literal gate walks);
        // look through it.
        BinaryOperator::Equal
            if matches!(
                proof_plan.program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            return recurse(binary.left);
        }
        BinaryOperator::Less => Some((binary.left, binary.right, false)),
        BinaryOperator::LessOrEqual => Some((binary.left, binary.right, true)),
        BinaryOperator::Greater => Some((binary.right, binary.left, false)),
        BinaryOperator::GreaterOrEqual => Some((binary.right, binary.left, true)),
        _ => None,
    };
    let Some((argument_side, bound_side, inclusive)) = normalized else {
        return false;
    };
    if expression_display_name(proof_plan, argument_side) != argument_label {
        return false;
    }
    let Some(bound) = typed_trees::dependent_ranges::symbolic_max_bound(
        &proof_plan.program.expression_table,
        bound_side,
    ) else {
        return false;
    };
    if bound.field.as_str() != max_field.as_str() {
        return false;
    }
    let implied_offset = if inclusive {
        Some(bound.offset)
    } else {
        bound.offset.checked_sub(1)
    };
    implied_offset.is_some_and(|implied| implied <= max_offset)
}

/// Route (b): the named field's OWN enforced literal range minimum -- an
/// argument bounded by `min(field) + offset` satisfies `field + offset` for
/// EVERY runtime value the field's store-enforced range admits. `None` when
/// the field is unranged or carries a non-Exact domain (whose range is
/// deliberately permissive and must never discharge a bound).
fn dependent_field_floor(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    max_field: &Identifier,
) -> Option<i64> {
    let program = &proof_plan.program;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == obligation.machine.as_str())?;
    let attached = machine.attached_data.as_ref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached.as_str())?;
    let field_type =
        crate::obligations::data_field_type_by_name(program, data, max_field.as_str())?;
    enforced_literal_range_minimum(program, field_type)
}

/// The literal Range minimum of a type reference's Constrained shells, ONLY
/// under an Exact arithmetic domain (a non-Exact range is deliberately
/// permissive -- probed live on the store side -- and must never discharge a
/// bound). Mirrors the checker crate's `enforced_range_of_type_reference`.
fn enforced_literal_range_minimum(
    program: &typed_trees::TypedTrees,
    handle: typed_trees::types::TypeReferenceHandle,
) -> Option<i64> {
    use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            enforced_literal_range_minimum(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = program.type_reference_table.constraints(*constraints);
            if constraints.iter().any(|constraint| {
                matches!(
                    constraint,
                    TypeConstraintNode::ArithmeticDomain(domain)
                        if *domain != numerics::arithmetic::ArithmeticDomain::Exact
                )
            }) {
                return None;
            }
            constraints
                .iter()
                .find_map(|constraint| match constraint {
                    TypeConstraintNode::Range { minimum, .. } => {
                        program.expression_table.constant_integer_value(*minimum)
                    }
                    _ => None,
                })
                .or_else(|| enforced_literal_range_minimum(program, *base_type))
        }
        _ => None,
    }
}

/// Route (b) for CALL arguments: same floor, machine resolved from the
/// obligation's (self-receiver) caller.
fn dependent_call_field_floor(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    max_field: &Identifier,
) -> Option<i64> {
    let program = &proof_plan.program;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == obligation.machine.as_str())?;
    let attached = machine.attached_data.as_ref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached.as_str())?;
    let field_type =
        crate::obligations::data_field_type_by_name(program, data, max_field.as_str())?;
    enforced_literal_range_minimum(program, field_type)
}

/// Route (c)'s soundness fence: the argument's own dependent atom speaks of
/// the field AT THIS STATE'S ENTRY, while the new obligation speaks of the
/// NEXT entry -- valid only if the field cannot have changed in between.
/// Conservative whole-state scan: assignments to the field and resolved
/// statement/value calls whose shared R5 frame overlaps it defeat the route.
/// Opaque calls remain fail-closed; the other discharge routes (guard / floor)
/// remain.
fn state_preserves_field(
    proof_plan: &ProofPlan,
    machine_name: &str,
    state_name: &str,
    field: &Identifier,
) -> bool {
    use typed_trees::statement::StatementNode;
    let program = &proof_plan.program;
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
    else {
        return false;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == state_name)
    else {
        return false;
    };
    let call_frames = validation::CallFrameResolver::new(program);
    let field_path = format!("self.{}", field.as_str());
    for statement in program.statement_table.statements(state.statement_nodes) {
        let Some(value_written) = call_frames
            .as_ref()
            .and_then(|frames| frames.statement_value_may_write_paths(machine, statement))
        else {
            return false;
        };
        if value_written
            .iter()
            .any(|written| validation::frame_paths_overlap(&field_path, written))
        {
            return false;
        }
        match statement {
            StatementNode::Assignment(assignment) => {
                if expression_mentions_field(proof_plan, assignment.target, field) {
                    return false;
                }
            }
            StatementNode::Call(call) => {
                let Some(written) = call_frames
                    .as_ref()
                    .and_then(|frames| frames.may_write_paths(machine, call))
                else {
                    return false;
                };
                if written
                    .iter()
                    .any(|written| validation::frame_paths_overlap(&field_path, written))
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

fn expression_mentions_field(
    proof_plan: &ProofPlan,
    expression: ExpressionHandle,
    field: &Identifier,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            member.member.as_str() == field.as_str()
                || expression_mentions_field(proof_plan, member.receiver, field)
        }
        ExpressionNode::Borrow(inner) => expression_mentions_field(proof_plan, inner.target, field),
        ExpressionNode::Indexed(indexed) => {
            expression_mentions_field(proof_plan, indexed.collection, field)
        }
        _ => false,
    }
}

fn sibling_len_from_constraints(constraints: &[ProofConstraint]) -> Option<(i64, i64)> {
    constraints.iter().find_map(|constraint| match constraint {
        ProofConstraint::IntegerRangeSiblingLenMax {
            minimum,
            max_offset,
            ..
        } => Some((*minimum, *max_offset)),
        _ => None,
    })
}

/// Guard route for the sibling-length upper half: a conjunct (through the
/// `== true` desugar) relating the ARGUMENT to `<sibling-arg>.len + k`,
/// tight enough for the declared offset (`arg < s.len` implies
/// `arg <= s.len - 1`, so strict needs `k-1 <= offset`). The receiver is
/// matched by display spelling against the (Mutable-stripped) sibling
/// argument.
fn guard_proves_sibling_len_upper(
    proof_plan: &ProofPlan,
    argument: ExpressionHandle,
    guard: &TransitionGuardNode,
    sibling_argument: ExpressionHandle,
    max_offset: i64,
) -> bool {
    let TransitionGuardNode::When(guard) = guard else {
        return false;
    };
    let argument_label = expression_display_name(proof_plan, argument);
    let sibling_label = expression_display_name(
        proof_plan,
        strip_mutable_handle(proof_plan, sibling_argument),
    );
    sibling_conjunct_proves(
        proof_plan,
        *guard,
        &argument_label,
        &sibling_label,
        max_offset,
    )
}

fn sibling_conjunct_proves(
    proof_plan: &ProofPlan,
    guard: ExpressionHandle,
    argument_label: &str,
    sibling_label: &str,
    max_offset: i64,
) -> bool {
    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(guard)
    else {
        return false;
    };
    let recurse = |handle: ExpressionHandle| {
        sibling_conjunct_proves(
            proof_plan,
            handle,
            argument_label,
            sibling_label,
            max_offset,
        )
    };
    let normalized = match binary.operator {
        BinaryOperator::And => return recurse(binary.left) || recurse(binary.right),
        BinaryOperator::Equal
            if matches!(
                proof_plan.program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            return recurse(binary.left);
        }
        BinaryOperator::Less => Some((binary.left, binary.right, false)),
        BinaryOperator::LessOrEqual => Some((binary.left, binary.right, true)),
        BinaryOperator::Greater => Some((binary.right, binary.left, false)),
        BinaryOperator::GreaterOrEqual => Some((binary.right, binary.left, true)),
        _ => None,
    };
    let Some((argument_side, bound_side, inclusive)) = normalized else {
        return false;
    };
    if expression_display_name(proof_plan, argument_side) != argument_label {
        return false;
    }
    let Some(bound) = typed_trees::dependent_ranges::sibling_len_bound(
        &proof_plan.program.expression_table,
        bound_side,
    )
    .map(|bound| (bound.sibling, bound.offset))
    .or_else(|| {
        // The guard names the CALLER's expression (`self.buf.len`), not the
        // callee's param -- recognize `<expr>.len + k` with the RECEIVER
        // display-matched below instead of the bare-name rule.
        len_of_expression_bound(proof_plan, bound_side)
    }) else {
        return false;
    };
    let (receiver_label, k) = (bound.0, bound.1);
    if receiver_label.as_str() != sibling_label {
        return false;
    }
    let implied = if inclusive { Some(k) } else { k.checked_sub(1) };
    implied.is_some_and(|implied| implied <= max_offset)
}

/// `<receiver-expr>.len [+/- k]` with the receiver rendered by display name
/// (an Identifier carrying the display spelling for comparison).
fn len_of_expression_bound(
    proof_plan: &ProofPlan,
    bound: ExpressionHandle,
) -> Option<(Identifier, i64)> {
    let table = &proof_plan.program.expression_table;
    let (len_expr, offset) = match table.expression(bound) {
        ExpressionNode::Binary(binary) => {
            let ExpressionNode::Integer(literal) = table.expression(binary.right) else {
                return None;
            };
            let magnitude = literal.value_i64()?;
            let offset = match binary.operator {
                BinaryOperator::Add => magnitude,
                BinaryOperator::Subtract => magnitude.checked_neg()?,
                _ => return None,
            };
            (binary.left, offset)
        }
        _ => (bound, 0),
    };
    let ExpressionNode::Member(member) = table.expression(len_expr) else {
        return None;
    };
    if member.member.as_str() != "len" {
        return None;
    }
    let receiver_label = expression_display_name(proof_plan, member.receiver);
    Some((Identifier::generated(receiver_label), offset))
}

fn strip_mutable_handle(proof_plan: &ProofPlan, expression: ExpressionHandle) -> ExpressionHandle {
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => strip_mutable_handle(proof_plan, inner.target),
        _ => expression,
    }
}

fn cannot_prove_dependent_call_bound(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    minimum: i64,
    max_field: &Identifier,
    max_offset: i64,
) -> Diagnostic {
    let bound_spelling = match max_offset {
        0 => format!("self.{max_field}"),
        offset if offset < 0 => format!("self.{max_field} - {}", -offset),
        offset => format!("self.{max_field} + {offset}"),
    };
    Diagnostic::error(format!(
        "cannot prove call argument `{}` satisfies dependent parameter `{}` for `{}` in `{}.{}`; expected {minimum}..={bound_spelling} -- a call has no co-located guard, so only an argument within the field's declared minimum discharges here; route the call through a guarded transition to relate them",
        expression_display_name(proof_plan, obligation.argument),
        obligation.parameter,
        obligation.target,
        obligation.machine,
        obligation.state,
    ))
}

fn cannot_prove_dependent_transition_bound(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    minimum: i64,
    max_field: &Identifier,
    max_offset: i64,
) -> Diagnostic {
    let bound_spelling = match max_offset {
        0 => format!("self.{max_field}"),
        offset if offset < 0 => format!("self.{max_field} - {}", -offset),
        offset => format!("self.{max_field} + {offset}"),
    };
    Diagnostic::error(format!(
        "cannot prove transition argument `{}` satisfies dependent parameter `{}` in `{}.{}`; expected {minimum}..={bound_spelling} -- relate them on the arm (`{} <= {bound_spelling}` or a `<` guard), or tighten the argument below the field's declared minimum",
        expression_display_name(proof_plan, obligation.argument),
        obligation.parameter,
        obligation.machine,
        obligation.state,
        expression_display_name(proof_plan, obligation.argument),
    ))
}

fn cannot_prove_bounded_transition_integer(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove transition argument `{}` satisfies bounded parameter `{}` in `{}.{}`; expected {}..={}",
        expression_display_name(proof_plan, obligation.argument),
        obligation.parameter,
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_assignment_integer(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove assignment value `{}` satisfies bounded target `{}` in `{}.{}`; expected {}..={}",
        expression_display_name(proof_plan, obligation.value),
        expression_display_name(proof_plan, obligation.target),
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_return_integer(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove return value `{}` satisfies bounded return type in `{}.{}`; expected {}..={}",
        expression_display_name(proof_plan, obligation.value),
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_initializer_integer(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove initializer `{}` satisfies bounded value `{}`; expected {}..={}",
        expression_display_name(proof_plan, obligation.value),
        obligation.owner,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_transition_float(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove transition argument `{}` satisfies bounded parameter `{}` in `{}.{}`; expected {}..={}",
        expression_display_name(proof_plan, obligation.argument),
        obligation.parameter,
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_assignment_float(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove assignment value `{}` satisfies bounded target `{}` in `{}.{}`; expected {}..={}",
        expression_display_name(proof_plan, obligation.value),
        expression_display_name(proof_plan, obligation.target),
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_return_float(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove return value `{}` satisfies bounded return type in `{}.{}`; expected {}..={}",
        expression_display_name(proof_plan, obligation.value),
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_initializer_float(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove initializer `{}` satisfies bounded value `{}`; expected {}..={}",
        expression_display_name(proof_plan, obligation.value),
        obligation.owner,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_transition_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove transition argument `{}` satisfies `{}` for bounded parameter `{}` in `{}.{}`",
        expression_display_name(proof_plan, obligation.argument),
        constraint,
        obligation.parameter,
        obligation.machine,
        obligation.state
    ))
}

fn cannot_prove_assignment_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove assignment value `{}` satisfies `{}` for bounded target `{}` in `{}.{}`",
        expression_display_name(proof_plan, obligation.value),
        constraint,
        expression_display_name(proof_plan, obligation.target),
        obligation.machine,
        obligation.state
    ))
}

fn cannot_prove_return_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove return value `{}` satisfies `{}` for bounded return type in `{}.{}`",
        expression_display_name(proof_plan, obligation.value),
        constraint,
        obligation.machine,
        obligation.state
    ))
}

fn cannot_prove_initializer_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove initializer `{}` satisfies `{}` for bounded value `{}`",
        expression_display_name(proof_plan, obligation.value),
        constraint,
        obligation.owner
    ))
}

fn expression_display_name(proof_plan: &ProofPlan, expression: ExpressionHandle) -> String {
    proof_plan.program.expression_table.display_name(expression)
}

fn cannot_prove_bounded_call_integer(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    let target = obligation
        .receiver
        .as_ref()
        .map(|receiver| format!("{receiver}.{}", obligation.target))
        .unwrap_or_else(|| obligation.target.to_string());

    Diagnostic::error(format!(
        "cannot prove call argument `{}` satisfies bounded parameter `{}` for `{}` in `{}.{}`; expected {}..={}",
        expression_display_name(proof_plan, obligation.argument),
        obligation.parameter,
        target,
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_call_float(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    target_range: FloatRange,
) -> Diagnostic {
    let target = obligation
        .receiver
        .as_ref()
        .map(|receiver| format!("{receiver}.{}", obligation.target))
        .unwrap_or_else(|| obligation.target.to_string());

    Diagnostic::error(format!(
        "cannot prove call argument `{}` satisfies bounded parameter `{}` for `{}` in `{}.{}`; expected {}..={}",
        expression_display_name(proof_plan, obligation.argument),
        obligation.parameter,
        target,
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_call_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    constraint: &str,
) -> Diagnostic {
    let target = obligation
        .receiver
        .as_ref()
        .map(|receiver| format!("{receiver}.{}", obligation.target))
        .unwrap_or_else(|| obligation.target.to_string());

    Diagnostic::error(format!(
        "cannot prove call argument `{}` satisfies `{}` for bounded parameter `{}` for `{}` in `{}.{}`",
        expression_display_name(proof_plan, obligation.argument),
        constraint,
        obligation.parameter,
        target,
        obligation.machine,
        obligation.state
    ))
}
