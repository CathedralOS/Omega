//! Integer ranges proved for the operands each bounded obligation names.

use crate::checker::assignment_stability::collect_stable_assignment_conditions;
use crate::checker::guards::{
    apply_assignment_guard, apply_handle_condition, apply_handle_guard, apply_source_condition,
    expressions_equivalent_for_proof, unwrap_true_guard_condition,
};
use crate::obligations::{
    BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
    BoundedStateReturnObligation, BoundedTransitionArgumentObligation, IntegerRange,
    ProofConstraint, ProofPlan, declared_integer_range, dehoisted_operand, integer_binary_range,
};
use arena::HandleSpan;
use numerics::bignum::BigInt;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::statement::TransitionGuardNode;

pub(crate) fn integer_range_for_transition_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
) -> Option<IntegerRange> {
    if let Some(range) =
        anonymous_integer_argument_range(proof_plan, obligation.argument, obligation.base_type)
    {
        return Some(range);
    }
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

pub(crate) fn guarded_integer_range_for_transition_argument(
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
pub(crate) fn apply_handle_condition_complement(
    proof_plan: &ProofPlan,
    mut range: IntegerRange,
    argument: ExpressionHandle,
    condition: ExpressionHandle,
) -> IntegerRange {
    let condition = unwrap_true_guard_condition(proof_plan, condition);
    if let ExpressionNode::Unary(unary) = proof_plan.program.expression_table.expression(condition)
        && unary.operator == UnaryOperator::LogicalNot
    {
        return apply_handle_condition(proof_plan, range, argument, unary.operand);
    }
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

pub(crate) fn integer_range_for_call_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
) -> Option<IntegerRange> {
    if let Some(range) =
        anonymous_integer_argument_range(proof_plan, obligation.argument, obligation.base_type)
    {
        return Some(range);
    }
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

fn anonymous_integer_argument_range(
    proof_plan: &ProofPlan,
    argument: ExpressionHandle,
    destination: typed_trees::types::TypeReferenceHandle,
) -> Option<IntegerRange> {
    let program = proof_plan.program;
    let primitive = program.primitive_type_reference(destination)?;
    let literal = validation::land_anonymous_integer_expression(
        program,
        argument,
        primitive,
        |expression| validation::has_anonymous_operator_meaning(program, expression),
    )?;
    integer_range_for_literal(&literal)
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

pub(crate) fn guarded_integer_range_for_assignment(
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

    // Entry facts survive independently: a write to `self.x` invalidates the
    // `x < limit` conjunct, not a disjoint `y > floor` conjunct. Only positive
    // conjunctions split; an OR or negation does not establish its children.
    // Each retained fact still passes the full guard/value dependency gate,
    // so `c = 100; c = c + 1` cannot reuse an entry fact `c < 100`.
    let mut conditions = Vec::new();
    if let Some(TransitionGuardNode::When(condition)) = &obligation.state_guard {
        collect_stable_assignment_conditions(
            proof_plan,
            obligation,
            *condition,
            context,
            &mut conditions,
        );
    }
    for condition in &conditions {
        let guard = TransitionGuardNode::When(*condition);
        range = apply_assignment_guard(proof_plan, range, obligation.value, &guard);
        range = guard_refined_binary_range(proof_plan, range, obligation.value, &guard);
    }
    if !conditions.is_empty() {
        // OPERAND-wise refold of a top-level binary value: each operand's
        // range = its DECLARED range (resolved at build time), with the guard
        // filling in one the declaration leaves unbounded -- `self.p +
        // self.dir` with `p: [0..=8]` declared and `dir` bounded only by the
        // incoming `dir >= 0 && dir <= 1`. The whole-value fold dies at build
        // time on the unranged operand, and `guard_refined_binary_range`
        // above is place-vs-LITERAL only, so neither reaches this shape.
        // Refold once with all surviving facts: separate lower and upper
        // bounds can jointly constrain an otherwise unbounded operand.
        if let Some(operands) = &obligation.binary_operands
            && let (Some(left), Some(right)) = (
                guard_narrowed_operand_range(
                    proof_plan,
                    obligation,
                    &conditions,
                    operands.left,
                    operands.left_range.clone(),
                ),
                guard_narrowed_operand_range(
                    proof_plan,
                    obligation,
                    &conditions,
                    operands.right,
                    operands.right_range.clone(),
                ),
            )
            && let Some(folded) = integer_binary_range(operands.operator, left, right)
        {
            range = IntegerRange {
                minimum: range.minimum.max(folded.minimum),
                maximum: range.maximum.min(folded.maximum),
            };
        }
    }

    // Nothing declared AND nothing narrowed: keep reporting "no range" rather
    // than a vacuous full-line interval.
    if declared.is_none() && range == neutral_range() {
        return None;
    }
    Some(range)
}

/// One guard-narrowed OPERAND range for the assignment refold. The whole-
/// operand match handles operands spelled exactly as the guard spells them
/// (`self.dir` under `dir >= 0`); a NESTED binary operand needs one more
/// refold, because the guard constrains the places INSIDE it, never the
/// operand itself. `(self.col - 28) % 8` under `col >= 28 && col < 60`
/// proves `local_x: [-4..=11]` only when `col` narrows inside the
/// subtraction: [28..=59] - 28 refolds to [0..=31], and the remainder fold
/// then yields [0..=7]. Skipping the recursion keeps the build-time
/// [-28..=35] fold, whose truncating-remainder [-7..=7] fails the target.
///
/// Every condition passed its dependency stability gate above, so a fact that
/// reaches an inner place is as sound here as on the outer operand; the
/// refold only intersects, and an operand the guard leaves at its declared
/// range refolds exactly what the build-time fold already claimed.
fn guard_narrowed_operand_range(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    conditions: &[ExpressionHandle],
    handle: ExpressionHandle,
    declared: Option<IntegerRange>,
) -> Option<IntegerRange> {
    let mut narrowed = declared.unwrap_or_else(neutral_range);
    for condition in conditions {
        narrowed = apply_source_condition(
            proof_plan,
            narrowed,
            handle,
            *condition,
            obligation.machine_symbol,
            obligation.state_guard_source,
        );
    }

    // R4: an ensures-witnessed OPERAND place clamps here; an unsigned place's
    // type floor supplies the lower end.
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

    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(handle)
    else {
        return (narrowed != neutral_range()).then_some(narrowed);
    };

    let Some((machine, state)) = proof_plan
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == obligation.machine_symbol)
        .and_then(|machine| {
            proof_plan
                .program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == obligation.state_symbol)
                .map(|state| (machine, state))
        })
    else {
        return (narrowed != neutral_range()).then_some(narrowed);
    };

    let nested = |sub_operand: ExpressionHandle| {
        let sub_operand = dehoisted_operand(proof_plan.program, state, sub_operand);
        guard_narrowed_operand_range(
            proof_plan,
            obligation,
            conditions,
            sub_operand,
            declared_integer_range(proof_plan.program, machine, state, sub_operand),
        )
    };
    if let (Some(left), Some(right)) = (nested(binary.left), nested(binary.right))
        && let Some(folded) = integer_binary_range(binary.operator, left, right)
    {
        narrowed = IntegerRange {
            minimum: narrowed.minimum.max(folded.minimum),
            maximum: narrowed.maximum.min(folded.maximum),
        };
    }
    (narrowed != neutral_range()).then_some(narrowed)
}

/// Invocation-local custody for assignment-range queries over one immutable
/// proof plan. Reusing it changes no frame result: the resolver's cache keys
/// bind exact call nodes and owning machines from the same typed program.
pub struct AssignmentRangeContext<'program> {
    pub(crate) program: &'program typed_trees::TypedTrees,
    call_frames: std::sync::OnceLock<Option<validation::CallFrameResolver<'program>>>,
}

impl<'program> AssignmentRangeContext<'program> {
    pub fn new(proof_plan: &ProofPlan<'program>) -> Self {
        Self {
            program: proof_plan.program,
            call_frames: std::sync::OnceLock::new(),
        }
    }

    pub(crate) fn call_frames(&self) -> Option<&validation::CallFrameResolver<'program>> {
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
    if !receiver.is_self_receiver() {
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

pub(crate) fn integer_range_for_return_value(
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

pub(crate) fn integer_range_for_initializer(
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

pub(crate) fn integer_range_from_constraints(
    constraints: &[ProofConstraint],
) -> Option<IntegerRange> {
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

pub(crate) fn type_constraints<'proof>(
    proof_plan: &'proof ProofPlan<'_>,
    constraints: HandleSpan<ProofConstraint>,
) -> &'proof [ProofConstraint] {
    proof_plan.type_constraints.span(constraints).unwrap_or(&[])
}

pub(crate) fn integer_literal_handle(
    proof_plan: &ProofPlan,
    expression: ExpressionHandle,
) -> Option<i64> {
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
