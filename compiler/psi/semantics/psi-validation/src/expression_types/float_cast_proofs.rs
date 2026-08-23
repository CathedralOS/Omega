use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

#[derive(Debug, Clone, Copy)]
struct FloatProofInterval {
    minimum: f64,
    maximum: f64,
}

pub(super) fn float_to_integer_cast_is_proven(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    value: ExpressionHandle,
    target: PrimitiveType,
) -> bool {
    let Some(interval) = float_expression_interval(program, machine, state, value) else {
        return false;
    };
    let bits = target.scalar_byte_size().unwrap_or(8) * 8;
    if target.is_signed_integer() {
        let limit = 2.0_f64.powi(bits as i32 - 1);
        interval.minimum >= -limit && interval.maximum < limit
    } else {
        interval.minimum >= 0.0 && interval.maximum < 2.0_f64.powi(bits as i32)
    }
}

fn float_expression_interval(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    expression: ExpressionHandle,
) -> Option<FloatProofInterval> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Float(literal) => {
            let value = literal.value();
            value.is_finite().then_some(FloatProofInterval {
                minimum: value,
                maximum: value,
            })
        }
        ExpressionNode::Mutable(inner) => {
            float_expression_interval(program, machine, state, *inner)
        }
        ExpressionNode::Unary(unary) => {
            float_expression_interval(program, machine, state, unary.operand)
        }
        ExpressionNode::Binary(binary) => {
            use psi_typed_trees::expression::BinaryOperator;
            let left = float_expression_interval(program, machine, state, binary.left)?;
            let right = float_expression_interval(program, machine, state, binary.right)?;
            let candidates = match binary.operator {
                BinaryOperator::Add => [
                    left.minimum + right.minimum,
                    left.minimum + right.maximum,
                    left.maximum + right.minimum,
                    left.maximum + right.maximum,
                ],
                BinaryOperator::Subtract => [
                    left.minimum - right.minimum,
                    left.minimum - right.maximum,
                    left.maximum - right.minimum,
                    left.maximum - right.maximum,
                ],
                BinaryOperator::Multiply => [
                    left.minimum * right.minimum,
                    left.minimum * right.maximum,
                    left.maximum * right.minimum,
                    left.maximum * right.maximum,
                ],
                BinaryOperator::Divide if right.minimum > 0.0 || right.maximum < 0.0 => [
                    left.minimum / right.minimum,
                    left.minimum / right.maximum,
                    left.maximum / right.minimum,
                    left.maximum / right.maximum,
                ],
                _ => return None,
            };
            candidates
                .iter()
                .all(|value| value.is_finite())
                .then(|| FloatProofInterval {
                    minimum: candidates.iter().copied().fold(f64::INFINITY, f64::min),
                    maximum: candidates.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                })
        }
        _ => float_place_interval(program, machine, state, expression),
    }
}

fn float_place_interval(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: Option<&psi_typed_trees::state::State>,
    expression: ExpressionHandle,
) -> Option<FloatProofInterval> {
    let declared = crate::places::declared_place_type_raw(program, machine, state, expression)
        .and_then(|type_reference| float_type_reference_interval(program, type_reference));
    let state = state?;
    let guarded = incoming_float_guard_interval(
        program,
        machine,
        state,
        &program.expression_table.display_name(expression),
        declared,
    );
    guarded.or(declared)
}

#[derive(Debug, Clone, Copy, Default)]
struct FloatGuardFact {
    minimum: Option<f64>,
    maximum: Option<f64>,
    finite: bool,
}

fn incoming_float_guard_interval(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    target_state: &psi_typed_trees::state::State,
    place: &str,
    declared: Option<FloatProofInterval>,
) -> Option<FloatProofInterval> {
    use psi_typed_trees::statement::{StatementNode, TransitionGuardNode};
    let mut entry_facts = Vec::new();
    for source_state in program.machine_states(machine) {
        for statement in program
            .statement_table
            .statements(source_state.statement_nodes)
        {
            match statement {
                StatementNode::Call(call) if call.target.as_str() == target_state.name.as_str() => {
                    return None;
                }
                StatementNode::Transition(transition) => {
                    if transition_target_names_state(program, transition.continuation, target_state)
                    {
                        return None;
                    }
                    if !transition_target_names_state(program, transition.target, target_state) {
                        continue;
                    }
                    let TransitionGuardNode::When(condition) = transition.guard else {
                        return None;
                    };
                    let fact = float_guard_fact(program, condition, place, true);
                    if !fact.finite {
                        return None;
                    }
                    let minimum = fact.minimum.or(declared.map(|interval| interval.minimum))?;
                    let maximum = fact.maximum.or(declared.map(|interval| interval.maximum))?;
                    if minimum > maximum {
                        return None;
                    }
                    entry_facts.push(FloatProofInterval { minimum, maximum });
                }
                _ => {}
            }
        }
    }
    if entry_facts.is_empty() {
        return None;
    }
    Some(FloatProofInterval {
        minimum: entry_facts
            .iter()
            .map(|fact| fact.minimum)
            .fold(f64::INFINITY, f64::min),
        maximum: entry_facts
            .iter()
            .map(|fact| fact.maximum)
            .fold(f64::NEG_INFINITY, f64::max),
    })
}

fn transition_target_names_state(
    program: &TypedTrees,
    target: psi_typed_trees::statement::TransitionTargetHandle,
    state: &psi_typed_trees::state::State,
) -> bool {
    if !target.is_valid() {
        return false;
    }
    let psi_typed_trees::statement::TransitionTargetNode::Named { path, .. } =
        program.statement_table.transition_target(target)
    else {
        return false;
    };
    program
        .statement_table
        .name_path_members(path.members)
        .last()
        .is_some_and(|name| name.as_str() == state.name.as_str())
}

fn float_guard_fact(
    program: &TypedTrees,
    expression: ExpressionHandle,
    place: &str,
    polarity: bool,
) -> FloatGuardFact {
    use psi_typed_trees::expression::BinaryOperator;
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return FloatGuardFact::default();
    };
    if binary.operator == BinaryOperator::Equal
        && let ExpressionNode::Boolean(arm) = program.expression_table.expression(binary.right)
    {
        return float_guard_fact(program, binary.left, place, polarity == *arm);
    }
    if polarity && binary.operator == BinaryOperator::And {
        let left = float_guard_fact(program, binary.left, place, true);
        let right = float_guard_fact(program, binary.right, place, true);
        return FloatGuardFact {
            minimum: option_max(left.minimum, right.minimum),
            maximum: option_min(left.maximum, right.maximum),
            finite: left.finite || right.finite,
        };
    }

    let left_name = program.expression_table.display_name(binary.left);
    let right_name = program.expression_table.display_name(binary.right);
    if binary.operator == BinaryOperator::Equal && left_name == place && right_name == place {
        return FloatGuardFact {
            finite: polarity,
            ..FloatGuardFact::default()
        };
    }

    let (literal, name_on_left) = if left_name == place {
        (float_constant_value(program, binary.right), true)
    } else if right_name == place {
        (float_constant_value(program, binary.left), false)
    } else {
        (None, true)
    };
    let Some(literal) = literal.filter(|literal| literal.is_finite()) else {
        return FloatGuardFact::default();
    };
    let mut operator = if name_on_left {
        binary.operator
    } else {
        flip_comparison(binary.operator)
    };
    if !polarity {
        let Some(negated) = negate_float_comparison(operator) else {
            return FloatGuardFact::default();
        };
        operator = negated;
    }
    match operator {
        BinaryOperator::Less => FloatGuardFact {
            maximum: Some(next_float_down(literal)),
            finite: true,
            ..FloatGuardFact::default()
        },
        BinaryOperator::LessOrEqual => FloatGuardFact {
            maximum: Some(literal),
            finite: true,
            ..FloatGuardFact::default()
        },
        BinaryOperator::Greater => FloatGuardFact {
            minimum: Some(next_float_up(literal)),
            finite: true,
            ..FloatGuardFact::default()
        },
        BinaryOperator::GreaterOrEqual => FloatGuardFact {
            minimum: Some(literal),
            finite: true,
            ..FloatGuardFact::default()
        },
        BinaryOperator::Equal => FloatGuardFact {
            minimum: Some(literal),
            maximum: Some(literal),
            finite: true,
        },
        _ => FloatGuardFact::default(),
    }
}

fn flip_comparison(
    operator: psi_typed_trees::expression::BinaryOperator,
) -> psi_typed_trees::expression::BinaryOperator {
    use psi_typed_trees::expression::BinaryOperator;
    match operator {
        BinaryOperator::Less => BinaryOperator::Greater,
        BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
        BinaryOperator::Greater => BinaryOperator::Less,
        BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
        other => other,
    }
}

fn negate_float_comparison(
    operator: psi_typed_trees::expression::BinaryOperator,
) -> Option<psi_typed_trees::expression::BinaryOperator> {
    use psi_typed_trees::expression::BinaryOperator;
    Some(match operator {
        BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
        BinaryOperator::LessOrEqual => BinaryOperator::Greater,
        BinaryOperator::Greater => BinaryOperator::LessOrEqual,
        BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
        _ => return None,
    })
}

fn option_max(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (value @ Some(_), None) | (None, value @ Some(_)) => value,
        (None, None) => None,
    }
}

fn option_min(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (value @ Some(_), None) | (None, value @ Some(_)) => value,
        (None, None) => None,
    }
}

fn next_float_up(value: f64) -> f64 {
    if value == -0.0 {
        return f64::from_bits(1);
    }
    let bits = value.to_bits();
    f64::from_bits(if value.is_sign_negative() {
        bits - 1
    } else {
        bits + 1
    })
}

fn next_float_down(value: f64) -> f64 {
    if value == 0.0 {
        return f64::from_bits((1u64 << 63) | 1);
    }
    let bits = value.to_bits();
    f64::from_bits(if value.is_sign_negative() {
        bits + 1
    } else {
        bits - 1
    })
}

fn float_type_reference_interval(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<FloatProofInterval> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            float_type_reference_interval(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .find_map(|constraint| match constraint {
                TypeConstraintNode::Range { minimum, maximum } => {
                    let minimum = float_constant_value(program, *minimum)?;
                    let maximum = float_constant_value(program, *maximum)?;
                    (minimum.is_finite() && maximum.is_finite() && minimum <= maximum)
                        .then_some(FloatProofInterval { minimum, maximum })
                }
                _ => None,
            })
            .or_else(|| float_type_reference_interval(program, *base_type)),
        _ => None,
    }
}

fn float_constant_value(program: &TypedTrees, expression: ExpressionHandle) -> Option<f64> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Float(literal) => Some(literal.value()),
        ExpressionNode::Integer(literal) => literal.value_i64().map(|value| value as f64),
        ExpressionNode::Unary(unary) => float_constant_value(program, unary.operand),
        ExpressionNode::Binary(binary) => {
            use psi_typed_trees::expression::BinaryOperator;
            let left = float_constant_value(program, binary.left)?;
            let right = float_constant_value(program, binary.right)?;
            Some(match binary.operator {
                BinaryOperator::Add => left + right,
                BinaryOperator::Subtract => left - right,
                BinaryOperator::Multiply => left * right,
                BinaryOperator::Divide if right != 0.0 => left / right,
                _ => return None,
            })
        }
        _ => None,
    }
}
