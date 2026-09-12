//! Bounds valid for every evaluation of immutable, builtin integer expressions.

use super::*;
use language_core::OperatorSpelling;
use symbols::SymbolHandle;

mod fields;

/// Bounds enforced by an exact owned integer type at storage boundaries.
/// References and atomic/policy carriers supply no invariant here. A caller
/// using a field type must separately establish its exact declaration identity.
pub fn enforced_integer_type_bounds(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(i64, i64)> {
    let primitive = exact_integer_primitive(program, type_reference)?;
    let carrier = primitive_range(primitive)?;
    let interval = enforced_declared_range(program, type_reference)
        .map_or(carrier, |range| range.intersect(carrier));
    let (low, high) = (interval.low?, interval.high?);
    (low <= high).then_some((low, high))
}

fn exact_integer_primitive(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let mut carrier_type = type_reference;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(carrier_type)
    {
        carrier_type = *base_type;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(carrier_type)
    else {
        return None;
    };
    let primitive = match program.symbols.builtin_type_atom(*symbol)? {
        symbols::BuiltinTypeAtom::U8 => PrimitiveType::U8,
        symbols::BuiltinTypeAtom::U16 => PrimitiveType::U16,
        symbols::BuiltinTypeAtom::U32 => PrimitiveType::U32,
        symbols::BuiltinTypeAtom::U64 => PrimitiveType::U64,
        symbols::BuiltinTypeAtom::I8 => PrimitiveType::I8,
        symbols::BuiltinTypeAtom::I16 => PrimitiveType::I16,
        symbols::BuiltinTypeAtom::I32 => PrimitiveType::I32,
        symbols::BuiltinTypeAtom::I64 => PrimitiveType::I64,
        _ => return None,
    };
    if program.arithmetic_domain_for_type_reference(type_reference) != ArithmeticDomain::Exact {
        return None;
    }
    Some(primitive)
}

/// Bound a literal or builtin arithmetic tree over exact immutable primitive
/// parameters or their direct owned integer fields. No initializer, caller flow
/// fact, callee body, or mutable place is read: the interval is valid independently
/// of the evaluation snapshot.
pub fn immutable_integer_expression_bounds(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(i64, i64)> {
    if !program
        .machine_states(machine)
        .iter()
        .any(|candidate| candidate.symbol == state.symbol)
    {
        return None;
    }
    let value = bounds(program, machine.symbol, Some(state), expression)?;
    Some((value.interval.low?, value.interval.high?))
}

/// Closed endpoints use the same carrier and operand-landing checks without
/// admitting any parameter, field, or flow-derived value as a static constant.
pub(crate) fn closed_integer_expression_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<i64> {
    let value = bounds(program, SymbolHandle::invalid(), None, expression)?;
    let (low, high) = (value.interval.low?, value.interval.high?);
    (low == high).then_some(low)
}

/// Retain one-sided carrier bounds when projecting an exact builtin guard.
/// An unrestricted u64 has a useful zero floor even though its ceiling does
/// not fit the interval engine's i64 endpoint representation.
pub(super) fn builtin_comparison_intervals(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(Interval, Interval)> {
    if !program
        .machine_states(machine)
        .iter()
        .any(|candidate| candidate.symbol == state.symbol)
    {
        return None;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    let spelling = match binary.operator {
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        _ => return None,
    };
    let left = bounds(program, machine.symbol, Some(state), binary.left)?;
    let right = bounds(program, machine.symbol, Some(state), binary.right)?;
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        machine.symbol,
        expression,
        spelling,
        &[left.type_reference, right.type_reference],
    )
    .then_some((left.interval, right.interval))
}

struct Bounds {
    interval: Interval,
    primitive: Option<PrimitiveType>,
    type_reference: Option<TypeReferenceHandle>,
}

fn type_bounds(program: &TypedTrees, type_reference: TypeReferenceHandle) -> Option<Bounds> {
    let primitive = exact_integer_primitive(program, type_reference)?;
    let carrier = primitive_range(primitive)?;
    Some(Bounds {
        interval: enforced_declared_range(program, type_reference)
            .map_or(carrier, |range| range.intersect(carrier)),
        primitive: Some(primitive),
        type_reference: Some(type_reference),
    })
}

fn bounds(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<Bounds> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    // Anonymous subtrees retain rational meaning until an operand lands. The
    // syntax-only integer folder would truncate division and erase landing.
    if let Some(evaluated) =
        crate::literals::anonymous_numeric_value(program, expression, &mut |expression| {
            crate::literals::has_anonymous_operator_meaning(program, expression)
        })
    {
        let value = evaluated.value.to_integer_exact()?.to_i64()?;
        return Some(Bounds {
            interval: Interval::constant(value),
            primitive: None,
            type_reference: None,
        });
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => {
            let interval = literal_interval(literal);
            let type_reference = if let Some(landing) = literal.landing() {
                if landing.domain != ArithmeticDomain::Exact {
                    return None;
                }
                Some(crate::operators::landed_integer_literal_type_reference(
                    program, expression,
                )?)
            } else {
                None
            };
            let primitive = match type_reference {
                Some(reference) => {
                    let primitive = exact_integer_primitive(program, reference)?;
                    // The projected u64 ceiling is unbounded in Interval;
                    // validate its literal payload against the real carrier.
                    if (primitive == PrimitiveType::U64 && literal.value_u64().is_none())
                        || !primitive_range(primitive)?.contains(interval)
                    {
                        return None;
                    }
                    Some(primitive)
                }
                None => None,
            };
            Some(Bounds {
                interval,
                primitive,
                type_reference,
            })
        }
        ExpressionNode::Name(path) if path.symbol.is_valid() && path.head_symbol == path.symbol => {
            let parameter = program
                .state_parameters(state?)
                .iter()
                .find(|parameter| parameter.symbol == path.symbol)?;
            if parameter.is_self || parameter.is_mutable || parameter.is_const {
                return None;
            }
            type_bounds(program, parameter.type_reference)
        }
        ExpressionNode::Member(_) => type_bounds(
            program,
            fields::type_reference(program, state?, expression)?,
        ),
        ExpressionNode::Binary(binary) => {
            let spelling = match binary.operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                BinaryOperator::Modulo => OperatorSpelling::Modulo,
                _ => return None,
            };
            let left = bounds(program, machine, state, binary.left)?;
            let right = bounds(program, machine, state, binary.right)?;
            // A context-free endpoint has no owning specialization to select.
            // Retained late-bound occurrences may still acquire a trait meaning.
            // ponytail: veto any matching specialization until endpoints carry
            // their owner directly; an unrelated match may conservatively refuse.
            if state.is_none()
                && program
                    .machine_specializations
                    .iter()
                    .any(|specialization| {
                        !typed_trees::operator::selected_trait_operator_meanings(
                            program,
                            specialization.instance,
                            spelling,
                            &[left.type_reference, right.type_reference],
                        )
                        .is_empty()
                    })
            {
                return None;
            }
            if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                machine,
                expression,
                spelling,
                &[left.type_reference, right.type_reference],
            ) {
                return None;
            }
            if left
                .primitive
                .zip(right.primitive)
                .is_some_and(|(left, right)| left != right)
            {
                return None;
            }
            let primitive = left.primitive.or(right.primitive)?;
            let carrier = primitive_range(primitive)?;
            // Anonymous operands land at the already-typed operation. A small
            // result is not evidence that an out-of-range operand can land.
            if !carrier.contains(left.interval) || !carrier.contains(right.interval) {
                return None;
            }
            // A later operation may produce small bounds, but cannot repair
            // an earlier Exact overflow hidden by the signed interval window.
            if primitive == PrimitiveType::U64
                && matches!(
                    binary.operator,
                    BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
                )
                && !super::unsigned_representability::binary_fits(
                    binary.operator,
                    left.interval,
                    right.interval,
                    None,
                    None,
                )
            {
                return None;
            }
            if matches!(
                binary.operator,
                BinaryOperator::Divide | BinaryOperator::Modulo
            ) && primitive.is_signed_integer()
                && left.interval.contains(Interval::constant(carrier.low?))
                && right.interval.contains(Interval::constant(-1))
            {
                // Exact signed remainder shares the quotient's MIN / -1
                // definedness obligation, even though its result would be zero.
                return None;
            }
            // The shared i64 interval engine cannot represent abs(i64::MIN).
            // Do not use its saturating divisor magnitude as an exact bound.
            if binary.operator == BinaryOperator::Modulo && right.interval.low == Some(i64::MIN) {
                return None;
            }
            let interval = match binary.operator {
                BinaryOperator::Add => left.interval.add(right.interval),
                BinaryOperator::Subtract => left.interval.subtract(right.interval),
                BinaryOperator::Multiply => left.interval.multiply(right.interval),
                BinaryOperator::Divide => left.interval.divide(right.interval),
                BinaryOperator::Modulo
                    if left.interval.low == left.interval.high
                        && right.interval.low == right.interval.high =>
                {
                    Interval::constant(left.interval.low?.checked_rem(right.interval.low?)?)
                }
                BinaryOperator::Modulo => left.interval.modulo(right.interval),
                _ => return None,
            };
            // A parent must still see this result's selected integer carrier.
            // Operand refinements do not survive the arithmetic operation.
            let mut result_type = left.type_reference.or(right.type_reference)?;
            while let TypeReferenceNode::Constrained { base_type, .. } =
                program.type_reference_table.type_reference(result_type)
            {
                result_type = *base_type;
            }
            carrier.contains(interval).then_some(Bounds {
                interval,
                primitive: Some(primitive),
                type_reference: Some(result_type),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
