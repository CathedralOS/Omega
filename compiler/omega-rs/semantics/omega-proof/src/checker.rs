use crate::obligations::{
    BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
    BoundedStateReturnObligation, BoundedTransitionArgumentObligation, ProofConstraint,
    ProofObligation, ProofPlan,
};
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::statement::TransitionGuardNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntegerRange {
    minimum: i64,
    maximum: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FloatRange {
    minimum: f64,
    maximum: f64,
}

pub fn check_proof_plan(proof_plan: &ProofPlan) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

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
                check_bounded_state_return(proof_plan, obligation, &mut diagnostics);
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
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_return_named_constraints(proof_plan, obligation, diagnostics);

    if let Some(target_range) =
        integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(value_range) = integer_range_for_return_value(proof_plan, obligation) else {
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
        ExpressionNode::Integer(value) => Some(IntegerRange {
            minimum: *value,
            maximum: *value,
        }),
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
        integer_range_for_transition_argument(proof_plan, obligation).unwrap_or(IntegerRange {
            minimum: i64::MIN,
            maximum: i64::MAX,
        });

    // Co-located: the arm's guard and its arguments evaluate at the SAME
    // dispatch, so the guard fact needs no stability gate here (collection
    // downgrades the guard when a sibling argument contains an opaque call).
    let range = apply_handle_guard(proof_plan, base, obligation.argument, &obligation.guard);
    guard_refined_binary_range(proof_plan, range, obligation.argument, &obligation.guard)
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
            let value = finite_float_literal(*value)?;
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
            let value = finite_float_literal(*value)?;
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
            let value = finite_float_literal(*value)?;
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
            let value = finite_float_literal(*value)?;
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
            let value = finite_float_literal(*value)?;
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
        ExpressionNode::Integer(value) => Some(IntegerRange {
            minimum: *value,
            maximum: *value,
        }),
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
        ExpressionNode::Integer(value) => Some(IntegerRange {
            minimum: *value,
            maximum: *value,
        }),
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
    let mut range = integer_range_for_assignment(proof_plan, obligation)?;

    // The incoming-edge guard held at STATE ENTRY; it still holds at this
    // assignment only if nothing earlier in the state could have changed what
    // it constrained (a prior write to a may-aliasing place, or any opaque
    // call). Without this gate, `transition c < 100 { true -> bump() }` with
    // `bump { c = 100; c = c + 1 }` would "prove" the second write.
    if let Some(guard) = &obligation.state_guard
        && assignment_guard_is_stable(proof_plan, obligation, guard)
    {
        range = apply_assignment_guard(proof_plan, range, obligation.value, guard);
        range = guard_refined_binary_range(proof_plan, range, obligation.value, guard);
    }

    Some(range)
}

/// Whether the incoming-edge guard's facts survive from state entry to THIS
/// assignment: every earlier statement in the state must be a transparent
/// (call-free) local/assignment whose written place is provably DISJOINT from
/// every place the guard condition or the assignment value reads. Prefix
/// member paths alias (`self.state` vs `self.state.count`); distinct roots do
/// not (`self.pixels[i]` vs `self.i` -- the render-loop shape stays provable).
/// Unresolvable shapes and calls are opaque: the guard is dropped (sound).
fn assignment_guard_is_stable(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    guard: &TransitionGuardNode,
) -> bool {
    use omega_typed_trees::statement::StatementNode;

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

    // Every place the guard fact (and the value it refines) depends on.
    let mut read_paths: Vec<Vec<String>> = Vec::new();
    if let TransitionGuardNode::When(condition) = guard {
        collect_read_place_paths(proof_plan, *condition, &mut read_paths);
    }
    collect_read_place_paths(proof_plan, obligation.value, &mut read_paths);

    for statement in program.statement_table.statements(state.statement_nodes) {
        match statement {
            StatementNode::Assignment(assignment) => {
                if assignment.target == obligation.target
                    && assignment.value == obligation.value
                {
                    // Reached the obligation's own assignment: everything
                    // before it was transparent and disjoint.
                    return true;
                }
                if expression_contains_call(proof_plan, assignment.value) {
                    return false;
                }
                let Some(written) = written_place_path(proof_plan, assignment.target) else {
                    return false;
                };
                if read_paths
                    .iter()
                    .any(|read| member_paths_may_alias(read, &written))
                {
                    return false;
                }
            }
            StatementNode::LocalData(local) => {
                if local.initial_value.is_valid()
                    && expression_contains_call(proof_plan, local.initial_value)
                {
                    return false;
                }
                // A `let` binds a fresh local name; it cannot alias a field
                // path, and same-name reads in `read_paths` would be the local
                // itself (rebinding kills the fact conservatively).
                let written = vec![local.name.as_str().to_owned()];
                if read_paths
                    .iter()
                    .any(|read| member_paths_may_alias(read, &written))
                {
                    return false;
                }
            }
            _ => return false,
        }
    }
    false
}

/// The member path a place expression READS or WRITES, for the aliasing check:
/// `self.state.count` -> [self, state, count]; an INDEXED place resolves to its
/// collection's path (a write anywhere inside the collection aliases the whole
/// collection, nothing else). `None` for shapes the walk cannot name (treated
/// as opaque by callers).
fn written_place_path(
    proof_plan: &ProofPlan,
    expression: ExpressionHandle,
) -> Option<Vec<String>> {
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => written_place_path(proof_plan, *inner),
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
        ExpressionNode::Mutable(inner) => collect_read_place_paths(proof_plan, *inner, paths),
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
        ExpressionNode::Mutable(inner) => expression_contains_call(proof_plan, *inner),
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
    let place_range = match (binary.operator, place_is_left) {
        (BinaryOperator::Add, _) => IntegerRange {
            minimum: range.minimum.saturating_sub(literal),
            maximum: range.maximum.saturating_sub(literal),
        },
        (BinaryOperator::Subtract, true) => IntegerRange {
            minimum: range.minimum.saturating_add(literal),
            maximum: range.maximum.saturating_add(literal),
        },
        (BinaryOperator::Subtract, false) => IntegerRange {
            minimum: literal.saturating_sub(range.maximum),
            maximum: literal.saturating_sub(range.minimum),
        },
        _ => return range,
    };
    let narrowed = apply_handle_condition(proof_plan, place_range, place, *condition);
    if narrowed == place_range {
        return range;
    }
    let refolded = match (binary.operator, place_is_left) {
        (BinaryOperator::Add, _) => IntegerRange {
            minimum: narrowed.minimum.saturating_add(literal),
            maximum: narrowed.maximum.saturating_add(literal),
        },
        (BinaryOperator::Subtract, true) => IntegerRange {
            minimum: narrowed.minimum.saturating_sub(literal),
            maximum: narrowed.maximum.saturating_sub(literal),
        },
        (BinaryOperator::Subtract, false) => IntegerRange {
            minimum: literal.saturating_sub(narrowed.maximum),
            maximum: literal.saturating_sub(narrowed.minimum),
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
        ExpressionNode::Integer(value) => Some(IntegerRange {
            minimum: *value,
            maximum: *value,
        }),
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
        ExpressionNode::Integer(value) => Some(IntegerRange {
            minimum: *value,
            maximum: *value,
        }),
        _ => None,
    }
}

fn integer_range_from_constraints(constraints: &[ProofConstraint]) -> Option<IntegerRange> {
    let mut range: Option<IntegerRange> = None;

    for constraint in constraints {
        let ProofConstraint::IntegerRange { minimum, maximum } = constraint else {
            continue;
        };

        let candidate = IntegerRange {
            minimum: *minimum,
            maximum: *maximum,
        };

        range = Some(match range {
            Some(existing) => IntegerRange {
                minimum: existing.minimum.max(candidate.minimum),
                maximum: existing.maximum.min(candidate.maximum),
            },
            None => candidate,
        });
    }

    for constraint in constraints {
        let ProofConstraint::Named(name) = constraint else {
            continue;
        };

        let implied = match name.as_str() {
            "non_negative" => Some(IntegerRange {
                minimum: 0,
                maximum: i64::MAX,
            }),
            "positive" => Some(IntegerRange {
                minimum: 1,
                maximum: i64::MAX,
            }),
            _ => None,
        };

        let Some(implied) = implied else {
            continue;
        };

        range = Some(match range {
            Some(existing) => IntegerRange {
                minimum: existing.minimum.max(implied.minimum),
                maximum: existing.maximum.min(implied.maximum),
            },
            None => implied,
        });
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

fn finite_float_literal(value: omega_typed_trees::expression::FloatLiteral) -> Option<f64> {
    let value = value.value();
    value.is_finite().then_some(value)
}

fn integer_literal_handle(proof_plan: &ProofPlan, expression: ExpressionHandle) -> Option<i64> {
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => Some(*value),
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
        minimum: range.minimum.max(lower_bound),
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

fn expressions_equivalent_for_proof(
    proof_plan: &ProofPlan,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    if left == right {
        return true;
    }

    match (
        proof_plan.program.expression_table.expression(left),
        proof_plan.program.expression_table.expression(right),
    ) {
        (ExpressionNode::Mutable(left), _) => {
            expressions_equivalent_for_proof(proof_plan, *left, right)
        }
        (_, ExpressionNode::Mutable(right)) => {
            expressions_equivalent_for_proof(proof_plan, left, *right)
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
        _ => false,
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

    match operator {
        BinaryOperator::Equal => {
            range.minimum = range.minimum.max(value);
            range.maximum = range.maximum.min(value);
        }
        BinaryOperator::Greater => range.minimum = range.minimum.max(value.saturating_add(1)),
        BinaryOperator::GreaterOrEqual => range.minimum = range.minimum.max(value),
        BinaryOperator::Less => range.maximum = range.maximum.min(value.saturating_sub(1)),
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

    match operator {
        BinaryOperator::Equal => {
            range.minimum = range.minimum.max(value);
            range.maximum = range.maximum.min(value);
        }
        BinaryOperator::Greater => range.maximum = range.maximum.min(value.saturating_sub(1)),
        BinaryOperator::GreaterOrEqual => range.maximum = range.maximum.min(value),
        BinaryOperator::Less => range.minimum = range.minimum.max(value.saturating_add(1)),
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
            ("finite", ExpressionNode::Float(value)) => finite_float_literal(*value).is_some(),
            ("finite", ExpressionNode::Integer(_)) => true,
            ("non_negative", ExpressionNode::Integer(value)) => *value >= 0,
            ("positive", ExpressionNode::Integer(value)) => *value > 0,
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
            "positive" => range.minimum > 0,
            "non_negative" => range.minimum >= 0,
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
            "positive" => range.minimum > 0,
            "non_negative" => range.minimum >= 0,
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
        ("finite", ExpressionNode::Float(value)) => finite_float_literal(*value).is_some(),
        ("exact", ExpressionNode::Integer(_)) => true,
        ("non_negative", ExpressionNode::Integer(value)) => *value >= 0,
        ("positive", ExpressionNode::Integer(value)) => *value > 0,
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
        "non_negative" => {
            integer_range_from_constraints(constraints).is_some_and(|range| range.minimum >= 0)
        }
        "positive" => {
            integer_range_from_constraints(constraints).is_some_and(|range| range.minimum > 0)
        }
        "wrapping" => false,
        _ => false,
    }
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
