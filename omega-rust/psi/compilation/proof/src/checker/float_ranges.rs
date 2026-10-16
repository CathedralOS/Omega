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
//! refuted comparison contributes only to the ordered part: NaN also makes
//! that comparison false. Keep unordered possibility until independent evidence
//! excludes it, including when that evidence follows a negated conjunct. Guard
//! effects and crossed writes must preserve the argument being constrained.

use crate::checker::guards::unwrap_true_guard_condition;
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
/// A missing base includes both the extended ordered line and NaN. Refuting an
/// ordered comparison narrows that line but retains NaN. Only an established
/// ordered interval can discharge a bounded argument; NaN-only intersections
/// are not unreachable paths. Prior exits contribute only across disjoint writes.
pub(crate) fn guarded_float_range_for_transition_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
) -> Option<FloatRange> {
    let base = float_range_for_transition_argument(proof_plan, obligation);
    let program = proof_plan.program;
    let (machine, state) =
        program.machine_state(obligation.machine_symbol, obligation.state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let current = statements.get(obligation.statement_index)?;
    let frames = validation::CallFrameResolver::new(program)?;
    let mut reads = Vec::new();
    super::assignment_stability::collect_read_place_paths(
        proof_plan,
        obligation.argument,
        &mut reads,
    );
    super::assignment_stability::collect_read_place_paths(
        proof_plan,
        dehoisted_operand(program, state, obligation.argument),
        &mut reads,
    );
    let preserves = |written: Option<Vec<String>>| {
        written.is_some_and(|written| {
            !super::assignment_stability::resolved_writes_overlap_reads(&written, &reads)
        })
    };
    // The guard and all sibling operands precede consumption. A complete,
    // disjoint frame permits useful effectful guards; an aliasing write does
    // not become harmless merely because the comparison appeared first.
    if !preserves(frames.statement_value_may_write_paths(machine, current)) {
        return None;
    }
    let mut range = FloatGuardRange {
        ordered: base.unwrap_or(FloatRange {
            minimum: f64::NEG_INFINITY,
            maximum: f64::INFINITY,
            maximum_inclusive: true,
        }),
        may_be_nan: base.is_none()
            && !type_constraints(proof_plan, obligation.argument_constraints)
                .iter()
                .any(|constraint| {
                    matches!(constraint,
                    ProofConstraint::Named(name) if name.as_str() == "finite")
                }),
    };
    range = apply_float_guard(
        proof_plan,
        range,
        obligation.argument,
        &obligation.guard,
        obligation.machine_symbol,
        obligation.state_symbol,
        obligation.statement_index,
    );
    for statement in statements[..obligation.statement_index].iter().rev() {
        if !preserves(frames.statement_value_may_write_paths(machine, statement)) {
            break;
        }
        match statement {
            StatementNode::Assignment(_) => {
                if !preserves(
                    frames
                        .assignment_write_frame(machine, statement)
                        .into_complete_paths(),
                ) {
                    break;
                }
            }
            StatementNode::Call(call) => {
                if !preserves(frames.may_write_paths(machine, call)) {
                    break;
                }
            }
            StatementNode::Transition(transition) => {
                if let TransitionGuardNode::When(condition) = transition.guard
                    && obligation.refuted_exit_guards.contains(&condition)
                {
                    range = apply_float_condition(
                        proof_plan,
                        range,
                        obligation.argument,
                        condition,
                        obligation.machine_symbol,
                        obligation.state_symbol,
                        obligation.statement_index,
                        true,
                    );
                }
            }
            StatementNode::LocalData(_) | StatementNode::Expression(_) => {}
            _ => break,
        }
    }
    (!range.may_be_nan && inhabited(range.ordered)).then_some(range.ordered)
}

#[derive(Clone, Copy)]
struct FloatGuardRange {
    ordered: FloatRange,
    may_be_nan: bool,
}

fn inhabited(range: FloatRange) -> bool {
    range.minimum < range.maximum || (range.minimum == range.maximum && range.maximum_inclusive)
}

fn apply_float_guard(
    proof_plan: &ProofPlan,
    range: FloatGuardRange,
    argument: ExpressionHandle,
    guard: &TransitionGuardNode,
    machine_symbol: symbols::SymbolHandle,
    source_state: symbols::SymbolHandle,
    statement_index: usize,
) -> FloatGuardRange {
    match guard {
        TransitionGuardNode::Always => range,
        TransitionGuardNode::When(condition) => apply_float_condition(
            proof_plan,
            range,
            argument,
            *condition,
            machine_symbol,
            source_state,
            statement_index,
            false,
        ),
    }
}

fn apply_float_condition(
    proof_plan: &ProofPlan,
    range: FloatGuardRange,
    argument: ExpressionHandle,
    condition: ExpressionHandle,
    machine_symbol: symbols::SymbolHandle,
    source_state: symbols::SymbolHandle,
    statement_index: usize,
    refute: bool,
) -> FloatGuardRange {
    let condition = dehoisted_guard_condition(
        proof_plan,
        machine_symbol,
        source_state,
        statement_index,
        condition,
    );
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
            statement_index,
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
            statement_index,
            false,
        );
        return apply_float_condition(
            proof_plan,
            range,
            argument,
            binary.right,
            machine_symbol,
            source_state,
            statement_index,
            false,
        );
    }
    let Some((machine, state)) = proof_plan
        .program
        .machine_state(machine_symbol, source_state)
    else {
        return range;
    };
    if !validation::has_builtin_bound_expression_meaning(
        proof_plan.program,
        machine,
        Some(state),
        condition,
    ) && !validation::sealed_float_comparison(
        proof_plan.program,
        machine,
        Some(state),
        condition,
        binary,
    ) {
        return range;
    }
    apply_float_comparison(
        proof_plan,
        range,
        argument,
        binary.operator,
        binary.left,
        binary.right,
        machine_symbol,
        source_state,
        statement_index,
        refute,
    )
}

fn apply_float_comparison(
    proof_plan: &ProofPlan,
    mut range: FloatGuardRange,
    argument: ExpressionHandle,
    operator: BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
    machine_symbol: symbols::SymbolHandle,
    source_state: symbols::SymbolHandle,
    statement_index: usize,
    refute: bool,
) -> FloatGuardRange {
    let Some((machine, state)) = proof_plan
        .program
        .machine_state(machine_symbol, source_state)
    else {
        return range;
    };
    let dehoist =
        |operand| stable_scalar_operand(proof_plan, machine, state, statement_index, operand);
    let (state_argument, state_left, state_right) =
        (dehoist(argument), dehoist(left), dehoist(right));
    // Canonical places retain binder, field and selector identities. Matching
    // spelling or independently authored calls is not the same observation.
    let same_argument = |expression| {
        same_float_place(
            proof_plan,
            machine_symbol,
            source_state,
            statement_index,
            state_argument,
            expression,
        )
    };
    let on_left = same_argument(state_left);
    let on_right = same_argument(state_right);
    if on_left && on_right {
        // Reflexive IEEE equality (or refuted inequality) excludes NaN, not
        // infinity. It carries no numerical endpoint by itself.
        if matches!(
            (operator, refute),
            (
                BinaryOperator::Equal
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::GreaterOrEqual,
                false
            ) | (BinaryOperator::NotEqual, true)
        ) {
            range.may_be_nan = false;
        }
        return range;
    }
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
    // Complement only the ordered component. A false relation also admits
    // NaN, whereas a true ordered relation or equality excludes it.
    let operator = if refute {
        match operator {
            BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
            BinaryOperator::LessOrEqual => BinaryOperator::Greater,
            BinaryOperator::Greater => BinaryOperator::LessOrEqual,
            BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
            BinaryOperator::NotEqual => {
                range.may_be_nan = false;
                BinaryOperator::Equal
            }
            _ => return range,
        }
    } else {
        if !matches!(
            operator,
            BinaryOperator::Equal
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Less
        ) {
            return range;
        }
        range.may_be_nan = false;
        operator
    };
    match operator {
        BinaryOperator::Equal => {
            raise_minimum(&mut range.ordered, literal);
            cap_maximum(&mut range.ordered, literal, true);
        }
        BinaryOperator::GreaterOrEqual => raise_minimum(&mut range.ordered, literal),
        BinaryOperator::Greater => {
            if literal == f64::INFINITY {
                range.ordered = empty_float_range();
            } else {
                raise_minimum(&mut range.ordered, literal.next_up());
            }
        }
        BinaryOperator::LessOrEqual => cap_maximum(&mut range.ordered, literal, true),
        BinaryOperator::Less => {
            if literal == f64::NEG_INFINITY {
                range.ordered = empty_float_range();
            } else {
                cap_maximum(&mut range.ordered, literal, false);
            }
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
    statement_index: usize,
    mut condition: ExpressionHandle,
) -> ExpressionHandle {
    let Some((machine, state)) = proof_plan
        .program
        .machine_state(machine_symbol, source_state)
    else {
        return condition;
    };
    let mut previous_declaration = statement_index;
    loop {
        let unwrapped = unwrap_true_guard_condition(proof_plan, condition);
        if unwrapped != condition {
            if !validation::has_builtin_decomposed_guard_meaning(
                proof_plan.program,
                machine,
                Some(state),
                condition,
            ) {
                return condition;
            }
            condition = unwrapped;
        }
        let Some((declaration, initializer)) =
            stable_initializer(proof_plan, machine, state, statement_index, condition)
        else {
            return condition;
        };
        if declaration >= previous_declaration {
            return condition;
        }
        previous_declaration = declaration;
        condition = initializer;
    }
}

/// Resolve stable scalar captures at this observation, including index captures.
/// Closed literals and exact place reads are values; arbitrary calls or
/// arithmetic recipes are not replayed to manufacture identity.
fn stable_scalar_operand(
    proof_plan: &ProofPlan,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    mut operand: ExpressionHandle,
) -> ExpressionHandle {
    let mut previous_declaration = statement_index;
    loop {
        let Some((declaration, initializer)) =
            stable_initializer(proof_plan, machine, state, statement_index, operand)
        else {
            return operand;
        };
        if declaration >= previous_declaration
            || !matches!(
                proof_plan.program.expression_table.expression(initializer),
                ExpressionNode::Name(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Integer(_)
                    | ExpressionNode::Float(_)
            )
        {
            return operand;
        }
        previous_declaration = declaration;
        operand = initializer;
    }
}

/// A saved Boolean or scalar copy may stand for its initializer only while
/// both that binding and every observed place retain their values. This is a
/// query-local snapshot check, not permission to replay arbitrary expressions.
fn stable_initializer(
    proof_plan: &ProofPlan,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<(usize, ExpressionHandle)> {
    let program = proof_plan.program;
    let initializer = dehoisted_condition(program, state, expression);
    if initializer == expression {
        return None;
    }
    let name = program.expression_table.name_path_node(expression)?;
    let (declaration, _) = program.state_local_declaration(state, name.symbol)?;
    if declaration >= statement_index {
        return None;
    }
    let mut reads = Vec::new();
    super::assignment_stability::collect_read_place_paths(proof_plan, expression, &mut reads);
    super::assignment_stability::collect_read_place_paths(proof_plan, initializer, &mut reads);
    let frames = validation::CallFrameResolver::new(program)?;
    let preserves = |written: Option<Vec<String>>| {
        written.is_some_and(|written| {
            !super::assignment_stability::resolved_writes_overlap_reads(&written, &reads)
        })
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    for statement in statements.get(declaration + 1..=statement_index)? {
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
            StatementNode::Transition(_)
            | StatementNode::LocalData(_)
            | StatementNode::Expression(_) => {}
            _ => return None,
        }
    }
    Some((declaration, initializer))
}

fn same_float_place(
    proof_plan: &ProofPlan,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let program = proof_plan.program;
    let Some((machine, state)) = program.machine_state(machine_symbol, state_symbol) else {
        return false;
    };
    // Canonical storage identity erases borrow recasts. Its bytes are not
    // proof that an integer view and a float view denote the same value.
    let observed_primitive = |expression| {
        validation::expression_result_type_reference(program, machine, state, expression)
            .or_else(|| {
                validation::declared_place_type_raw(program, machine, Some(state), expression)
            })
            .and_then(|reference| program.primitive_type_reference(reference))
    };
    let left_primitive = observed_primitive(left);
    if !matches!(
        left_primitive,
        Some(PrimitiveType::F32 | PrimitiveType::F64)
    ) || left_primitive != observed_primitive(right)
    {
        return false;
    }
    let canonical = |expression| {
        // Authenticate the original coordinates before canonicalization can
        // fold a selector's spelling (such as authored `0 + 0`) into an index.
        if !validation::place_has_builtin_coordinates(program, machine, Some(state), expression) {
            return None;
        }
        let mut place = validation::canonical_place_from_expression(program, expression)?;
        for segment in &mut place.segments {
            if let facts::PlaceSegment::Index { expression } = segment {
                let operand =
                    stable_scalar_operand(proof_plan, machine, state, statement_index, *expression);
                *segment = validation::index_place_segment(program, operand);
            }
        }
        Some(place)
    };
    let (Some(left), Some(right)) = (canonical(left), canonical(right)) else {
        return false;
    };
    left.root == right.root
        && left.segments.len() == right.segments.len()
        && left
            .segments
            .iter()
            .zip(&right.segments)
            .all(|(left, right)| match (left, right) {
                (
                    facts::PlaceSegment::Index { expression: left },
                    facts::PlaceSegment::Index { expression: right },
                ) => {
                    validation::has_builtin_bound_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        *left,
                    ) && validation::has_builtin_bound_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        *right,
                    ) && program
                        .expression_table
                        .expressions_structurally_equal(*left, *right)
                }
                _ => left == right,
            })
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
