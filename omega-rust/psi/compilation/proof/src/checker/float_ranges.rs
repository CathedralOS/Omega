//! Float ranges proved for the operands each bounded obligation names.
//!
//! A declaration describes valid boundary values, not every intermediate
//! store inside an invariant window. At an observation, a surviving literal
//! store can establish the current value independently of its declared domain.
//! The backward query crosses only complete, disjoint mutation frames and
//! includes sibling operand effects at consumption. It neither imports a
//! later repair nor transports a valuation across a state boundary.
//!
//! A co-located transition guard is not a stored valuation. IEEE ordered
//! comparisons are false for NaN, so a true arm proves an ordered interval
//! about the compared place: `difference >= 0.5 && difference <= 8.0` is
//! exactly the declared range `f64 [0.5..=8.0]`. The same interval is what a
//! later refuted exit guard contributes, with the comparison inverted. An
//! unsatisfiable strict bound (`value > +inf`) stays empty and proves nothing.

use crate::checker::guards::{expressions_equivalent_for_proof, unwrap_true_guard_condition};
use crate::checker::integer_ranges::type_constraints;
use crate::obligations::{
    BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
    BoundedStateReturnObligation, BoundedTransitionArgumentObligation, FloatRange, ProofConstraint,
    ProofPlan,
};
use crate::typed_expressions::{dehoisted_condition, dehoisted_operand};
use arena::HandleSpan;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::statement::{StatementNode, TransitionGuardNode};
use typed_trees::types::PrimitiveType;

macro_rules! float_range_for {
    ($name:ident, $obligation:ty, $expression:ident, $constraints:ident) => {
        pub(crate) fn $name(
            proof_plan: &ProofPlan,
            obligation: &$obligation,
        ) -> Option<FloatRange> {
            float_range(
                proof_plan,
                obligation.machine_symbol,
                obligation.state_symbol,
                obligation.statement_index,
                obligation.$expression,
                obligation.$constraints,
            )
        }
    };
}

float_range_for!(
    float_range_for_transition_argument,
    BoundedTransitionArgumentObligation,
    argument,
    argument_constraints
);
float_range_for!(
    float_range_for_assignment,
    BoundedAssignmentObligation,
    value,
    value_constraints
);
float_range_for!(
    float_range_for_call_argument,
    BoundedCallArgumentObligation,
    argument,
    argument_constraints
);
float_range_for!(
    float_range_for_return_value,
    BoundedStateReturnObligation,
    value,
    value_constraints
);

/// The literal-or-consulted float range spine every per-role
/// `float_range_for_*` shares — the role wrappers read out their own
/// operand and constraint fields and delegate here.
fn float_range(
    proof_plan: &ProofPlan,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    operand: ExpressionHandle,
    constraints: HandleSpan<ProofConstraint>,
) -> Option<FloatRange> {
    match proof_plan.program.expression_table.expression(operand) {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange::closed(value))
        }
        _ => float_range_at(
            proof_plan,
            machine_symbol,
            state_symbol,
            statement_index,
            operand,
        )
        .or_else(|| float_range_from_constraints(type_constraints(proof_plan, constraints))),
    }
}

/// A reaching literal describes current storage, not its declared invariant.
/// Only resolved scalar places participate. Every crossed effect is checked
/// with the ordinary alias-aware write frames; incomplete frames lose the fact.
fn float_range_at(
    proof_plan: &ProofPlan,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<FloatRange> {
    let program = proof_plan.program;
    let (machine, state) = program.machine_state(machine_symbol, state_symbol)?;
    let identity = resolved_place_identity(program, expression)?;
    let place_type =
        validation::declared_place_type_raw(program, machine, Some(state), expression)?;
    let primitive = program.primitive_type_reference(place_type)?;
    let format = match primitive {
        PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
        PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
        _ => return None,
    };
    let path = super::assignment_stability::written_place_path(proof_plan, expression)?.join(".");
    let frames = validation::CallFrameResolver::new(program)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let current = statements.get(statement_index)?;
    let root = *identity.first()?;
    let reference_local = statements.iter().find_map(|statement| {
        let local = statement.as_local_data()?;
        if local.symbol != root {
            return None;
        }
        let mut reference = local.type_reference;
        while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            reference = *base_type;
        }
        matches!(
            program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::Reference { .. }
        )
        .then_some(local.symbol)
    });
    // A conservative write footprint is not an exact referent identity.
    // Divergent or coarse local origins cannot supply a reaching-store fact.
    let observed_origin = match reference_local {
        Some(local) => {
            Some(frames.local_reference_origin_before_statement(machine, current, local)?)
        }
        None => None,
    };
    // Include all evaluation effects, not just the selected argument: a sibling
    // argument or guard can overwrite the observed place before consumption.
    let preserves = |written: Option<Vec<String>>| {
        written.is_some_and(|written| {
            written
                .iter()
                .all(|written| !validation::frame_paths_overlap(&path, written))
        })
    };
    if !preserves(frames.statement_value_may_write_paths(machine, current)) {
        return None;
    }
    for statement in statements[..statement_index].iter().rev() {
        // A local's initializer is its first store; nothing reaches past it.
        if let Some(local) = statement.as_local_data()
            && identity.as_slice() == [local.symbol]
        {
            if program.primitive_type_reference(local.type_reference) != Some(primitive) {
                return None;
            }
            let ExpressionNode::Float(literal) =
                program.expression_table.expression(local.initial_value)
            else {
                return None;
            };
            return (literal.landing() == Some(format))
                .then(|| finite_float_literal(literal))
                .flatten()
                .map(FloatRange::closed);
        }
        if let Some(assignment) = statement.as_assignment()
            && resolved_place_identity(program, assignment.target).as_ref() == Some(&identity)
        {
            if let Some(local) = reference_local
                && frames.local_reference_origin_before_statement(machine, statement, local)
                    != observed_origin
            {
                return None;
            }
            let target_type = validation::declared_place_type_raw(
                program,
                machine,
                Some(state),
                assignment.target,
            )?;
            if program.primitive_type_reference(target_type) != Some(primitive) {
                return None;
            }
            let ExpressionNode::Float(literal) =
                program.expression_table.expression(assignment.value)
            else {
                return None;
            };
            return (literal.landing() == Some(format))
                .then(|| finite_float_literal(literal))
                .flatten()
                .map(FloatRange::closed);
        }
        if !preserves(frames.statement_value_may_write_paths(machine, statement)) {
            return None;
        }
        match statement {
            StatementNode::Assignment(_) => {
                if !preserves(
                    frames
                        .assignment_write_frame(machine, statement)
                        .into_complete_paths(),
                ) {
                    return None;
                }
            }
            StatementNode::Call(call) => {
                if !preserves(frames.may_write_paths(machine, call)) {
                    return None;
                }
            }
            StatementNode::LocalData(_) | StatementNode::Expression(_) => {}
            _ => return None,
        }
    }
    None
}

fn resolved_place_identity(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<Vec<symbols::SymbolHandle>> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) if validation::is_single_segment_name_path(program, path) => {
            Some(vec![path.symbol])
        }
        ExpressionNode::Member(member) if member.member_symbol.is_valid() => {
            let mut identity = resolved_place_identity(program, member.receiver)?;
            identity.push(member.member_symbol);
            Some(identity)
        }
        _ => None,
    }
}

/// The float interval a co-located guard proves about `argument`.
///
/// `base` is the interval already proved from the place's type or a reaching
/// literal. A missing base starts at the whole extended line, which a finite
/// declared range does not contain until a comparison cuts it down. Prior
/// exit guards apply with the comparison inverted: reaching this arm refuted
/// them. An empty result proves nothing.
pub(crate) fn guarded_float_range(
    proof_plan: &ProofPlan,
    base: Option<FloatRange>,
    argument: ExpressionHandle,
    guard: &TransitionGuardNode,
    refuted_exit_guards: &[ExpressionHandle],
    machine_symbol: symbols::SymbolHandle,
    source_state: symbols::SymbolHandle,
) -> Option<FloatRange> {
    let mut range = base.unwrap_or(FloatRange {
        minimum: f64::NEG_INFINITY,
        maximum: f64::INFINITY,
        maximum_inclusive: true,
    });
    range = apply_float_guard(
        proof_plan,
        range,
        argument,
        guard,
        machine_symbol,
        source_state,
    );
    for refuted in refuted_exit_guards {
        range = apply_float_condition(
            proof_plan,
            range,
            argument,
            *refuted,
            machine_symbol,
            source_state,
            true,
        );
    }
    inhabited(range).then_some(range)
}

fn inhabited(range: FloatRange) -> bool {
    range.minimum < range.maximum || (range.minimum == range.maximum && range.maximum_inclusive)
}

fn apply_float_guard(
    proof_plan: &ProofPlan,
    range: FloatRange,
    argument: ExpressionHandle,
    guard: &TransitionGuardNode,
    machine_symbol: symbols::SymbolHandle,
    source_state: symbols::SymbolHandle,
) -> FloatRange {
    match guard {
        TransitionGuardNode::Always => range,
        TransitionGuardNode::When(condition) => apply_float_condition(
            proof_plan,
            range,
            argument,
            *condition,
            machine_symbol,
            source_state,
            false,
        ),
    }
}

fn apply_float_condition(
    proof_plan: &ProofPlan,
    range: FloatRange,
    argument: ExpressionHandle,
    condition: ExpressionHandle,
    machine_symbol: symbols::SymbolHandle,
    source_state: symbols::SymbolHandle,
    refute: bool,
) -> FloatRange {
    let condition = dehoisted_guard_condition(proof_plan, machine_symbol, source_state, condition);
    let condition = unwrap_true_guard_condition(proof_plan, condition);
    if let ExpressionNode::Unary(unary) = proof_plan.program.expression_table.expression(condition)
        && unary.operator == UnaryOperator::LogicalNot
    {
        return apply_float_condition(
            proof_plan,
            range,
            argument,
            unary.operand,
            machine_symbol,
            source_state,
            !refute,
        );
    }
    let Some(binary) = proof_plan.program.expression_table.binary_node(condition) else {
        return range;
    };
    if binary.operator == BinaryOperator::And {
        // A refuted conjunction does not say which leg failed.
        if refute {
            return range;
        }
        let range = apply_float_condition(
            proof_plan,
            range,
            argument,
            binary.left,
            machine_symbol,
            source_state,
            false,
        );
        return apply_float_condition(
            proof_plan,
            range,
            argument,
            binary.right,
            machine_symbol,
            source_state,
            false,
        );
    }
    let operator = if refute {
        match binary.operator {
            BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
            BinaryOperator::LessOrEqual => BinaryOperator::Greater,
            BinaryOperator::Greater => BinaryOperator::LessOrEqual,
            BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
            _ => return range,
        }
    } else {
        binary.operator
    };
    apply_float_comparison(
        proof_plan,
        range,
        argument,
        operator,
        binary.left,
        binary.right,
        machine_symbol,
        source_state,
    )
}

fn apply_float_comparison(
    proof_plan: &ProofPlan,
    mut range: FloatRange,
    argument: ExpressionHandle,
    operator: BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    machine_symbol: symbols::SymbolHandle,
    source_state: symbols::SymbolHandle,
) -> FloatRange {
    let (state_argument, state_left, state_right) = dehoisted_comparison_sides(
        proof_plan,
        machine_symbol,
        source_state,
        argument,
        left,
        right,
    );
    let on_left = expressions_equivalent_for_proof(proof_plan, state_left, state_argument);
    let on_right = expressions_equivalent_for_proof(proof_plan, state_right, state_argument);
    let (operator, literal) = if on_left {
        (operator, float_literal_value(proof_plan, state_right))
    } else if on_right {
        (
            flip_comparison(operator),
            float_literal_value(proof_plan, state_left),
        )
    } else {
        return range;
    };
    let Some(literal) = literal else {
        return range;
    };
    match operator {
        BinaryOperator::Equal => {
            raise_minimum(&mut range, literal);
            cap_maximum(&mut range, literal, true);
        }
        BinaryOperator::GreaterOrEqual => raise_minimum(&mut range, literal),
        BinaryOperator::Greater => {
            if literal == f64::INFINITY {
                return empty_float_range();
            }
            raise_minimum(&mut range, literal.next_up());
        }
        BinaryOperator::LessOrEqual => cap_maximum(&mut range, literal, true),
        BinaryOperator::Less => {
            if literal == f64::NEG_INFINITY {
                return empty_float_range();
            }
            cap_maximum(&mut range, literal, false);
        }
        _ => {}
    }
    range
}

fn flip_comparison(operator: BinaryOperator) -> BinaryOperator {
    match operator {
        BinaryOperator::Less => BinaryOperator::Greater,
        BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
        BinaryOperator::Greater => BinaryOperator::Less,
        BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
        other => other,
    }
}

fn dehoisted_guard_condition(
    proof_plan: &ProofPlan,
    machine_symbol: symbols::SymbolHandle,
    source_state: symbols::SymbolHandle,
    condition: ExpressionHandle,
) -> ExpressionHandle {
    let Some((machine, state)) = proof_plan
        .program
        .machine_state(machine_symbol, source_state)
    else {
        return condition;
    };
    let _ = machine;
    dehoisted_condition(proof_plan.program, state, condition)
}

fn dehoisted_comparison_sides(
    proof_plan: &ProofPlan,
    machine_symbol: symbols::SymbolHandle,
    source_state: symbols::SymbolHandle,
    argument: ExpressionHandle,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> (ExpressionHandle, ExpressionHandle, ExpressionHandle) {
    let Some((_, state)) = proof_plan
        .program
        .machine_state(machine_symbol, source_state)
    else {
        return (argument, left, right);
    };
    (
        dehoisted_operand(proof_plan.program, state, argument),
        dehoisted_operand(proof_plan.program, state, left),
        dehoisted_operand(proof_plan.program, state, right),
    )
}

fn float_literal_value(proof_plan: &ProofPlan, expression: ExpressionHandle) -> Option<f64> {
    let ExpressionNode::Float(value) = proof_plan.program.expression_table.expression(expression)
    else {
        return None;
    };
    let value = value.landed_f64();
    (!value.is_nan()).then_some(value)
}

fn raise_minimum(range: &mut FloatRange, bound: f64) {
    if bound > range.minimum {
        range.minimum = bound;
    }
}

fn cap_maximum(range: &mut FloatRange, bound: f64, inclusive: bool) {
    if bound < range.maximum {
        range.maximum = bound;
        range.maximum_inclusive = inclusive;
    } else if bound == range.maximum {
        range.maximum_inclusive = range.maximum_inclusive && inclusive;
    }
}

fn empty_float_range() -> FloatRange {
    FloatRange {
        minimum: 1.0,
        maximum: 0.0,
        maximum_inclusive: true,
    }
}

#[cfg(test)]
mod tests;

pub(crate) fn float_range_for_initializer(
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
            Some(FloatRange::closed(value))
        }
        _ => obligation
            .owner
            .state_statement()
            .and_then(|(machine_symbol, state_symbol, statement_index)| {
                float_range_at(
                    proof_plan,
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    obligation.value,
                )
            })
            .or_else(|| {
                float_range_from_constraints(type_constraints(
                    proof_plan,
                    obligation.value_constraints,
                ))
            }),
    }
}

pub(crate) fn float_range_from_constraints(constraints: &[ProofConstraint]) -> Option<FloatRange> {
    crate::obligations::float_range_from_constraints(constraints)
}

pub(crate) fn finite_float_literal(value: &typed_trees::expression::FloatLiteral) -> Option<f64> {
    // The riding landing decides the value: `0.3f32` is the widened f32, not
    // the f64 read of its spelling (and f32 overflow is genuinely infinite).
    let value = value.landed_f64();
    value.is_finite().then_some(value)
}

pub(crate) fn float_range_display(range: FloatRange) -> String {
    if range.maximum_inclusive {
        format!("expected {}..={}", range.minimum, range.maximum)
    } else {
        format!("expected {}..{}", range.minimum, range.maximum)
    }
}
