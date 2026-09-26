//! The existing declaration/custody owner, not token spelling, admits arithmetic.
use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, Machine, PrimitiveType, State, TypedTrees,
    fields, lengths,
};
use language_core::operator_spelling::OperatorSpelling;
use typed_trees::expression::UnaryOperator;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn builtin(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<Option<TypeReferenceHandle>> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    if super::super::proof_integer::anonymous_integer_value(program, expression).is_some() {
        // This uses the existing anonymous-operator classifier and folds only
        // a wholly anonymous integral subtree, not typed integer division.
        return Some(None);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => Some(
            program
                .closed_integer_value_in(expression, machine.symbol)?
                .type_reference,
        ),
        ExpressionNode::Boolean(_) => Some(None),
        ExpressionNode::Name(path) if path.symbol.is_valid() && path.head_symbol == path.symbol => {
            let parameter = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == path.symbol)?;
            // Meaning is independent of binding stability. State and call
            // edge owners separately establish exact prefix preservation,
            // including for mutable parameters.
            (!parameter.is_self && !parameter.is_const).then_some(Some(parameter.type_reference))
        }
        ExpressionNode::Atomic(atomic) => builtin(program, machine, state, atomic.value, depth + 1),
        ExpressionNode::Member(_) => {
            // An exact declared projection chain, rooted at a formal that
            // names its record directly or through a reference, has builtin
            // meaning at every step, whatever its leaf's carrier.
            if let Some(reference) = fields::projected_type(program, state, expression) {
                return Some(Some(reference));
            }
            let receiver = crate::value_custody::places::collection_length_receiver(
                program,
                machine,
                Some(state),
                expression,
            )?;
            // A `.len` receiver is a slice formal or an exact projected slice
            // leaf; either names a produced length coordinate.
            if lengths::parameter(program, state, receiver).is_none()
                && lengths::SliceCoordinate::resolve(program, state, receiver).is_none()
            {
                return None;
            }
            // Builtin collection metadata is a natural numeric coordinate;
            // it has no authored nominal operator implementation.
            Some(None)
        }
        ExpressionNode::StructLiteral(literal) => {
            if !literal.type_symbol.is_valid() {
                return None;
            }
            for field in program.expression_table.struct_fields(literal.fields) {
                builtin(program, machine, state, field.value, depth + 1)?;
            }
            // Constructor operands keep their resolved nominal carrier when
            // checking an enclosing operator's selected declaration.
            Some(Some(
                program
                    .type_reference_table
                    .find_named_type_reference(literal.type_symbol)?,
            ))
        }
        ExpressionNode::Indexed(indexed) => {
            // A builtin subslice's collection is a slice formal or an exact
            // projected slice leaf reached through a member chain.
            let collection_type = match lengths::parameter(program, state, indexed.collection) {
                Some(parameter) => parameter.type_reference,
                None => {
                    lengths::SliceCoordinate::resolve(program, state, indexed.collection)?
                        .field
                        .type_reference
                }
            };
            if !crate::value_custody::places::has_builtin_subslice_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) {
                return None;
            }
            let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index)
            else {
                return None;
            };
            for endpoint in [range.start, range.end] {
                if endpoint.is_valid() {
                    builtin(program, machine, state, endpoint, depth + 1)?;
                }
            }
            Some(Some(collection_type))
        }
        ExpressionNode::Borrow(borrow) => {
            // A borrow performs no operation of its own; the referent's
            // meaning is what the edge substitutes into a borrowed formal.
            builtin(program, machine, state, borrow.target, depth + 1)?;
            Some(None)
        }
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            builtin(program, machine, state, unary.operand, depth + 1)?;
            Some(None)
        }
        ExpressionNode::Binary(binary) => {
            let left = builtin(program, machine, state, binary.left, depth + 1)?;
            let right = builtin(program, machine, state, binary.right, depth + 1)?;
            // `<<`/`>>` name no overloadable spelling: a typed occurrence is
            // the builtin operator. Its carrier is the shifted operand's own
            // type; the count is an independent integer operand whose
            // primitive need not agree with it. Carrier-less counts such as
            // a slice length remain admissible operands, as for division.
            if matches!(
                binary.operator,
                BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
            ) {
                let integer_carrier = |carrier| {
                    program
                        .primitive_type_reference(carrier)
                        .is_some_and(PrimitiveType::accepts_integer_literal)
                };
                return match left {
                    Some(left) if integer_carrier(left) && right.is_none_or(integer_carrier) => {
                        Some(Some(left))
                    }
                    _ => None,
                };
            }
            // `&` also names no overloadable spelling: a typed occurrence is
            // the builtin operator. Unlike a shift both operands share one
            // carrier -- an anonymous literal or natural coordinate is the
            // same wildcard input division's operands are -- and the
            // operation is total inside it, so meaning keeps that agreed
            // integer primitive as the result carrier.
            if binary.operator == BinaryOperator::BitwiseAnd {
                if left.zip(right).is_some_and(|(left, right)| {
                    program.primitive_type_reference(left)
                        != program.primitive_type_reference(right)
                }) {
                    return None;
                }
                return match left.or(right) {
                    Some(carrier)
                        if program
                            .primitive_type_reference(carrier)
                            .is_some_and(PrimitiveType::accepts_integer_literal) =>
                    {
                        Some(Some(carrier))
                    }
                    _ => None,
                };
            }
            let spelling = match binary.operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                BinaryOperator::Modulo => OperatorSpelling::Modulo,
                BinaryOperator::Equal => OperatorSpelling::Equal,
                BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
                BinaryOperator::Less => OperatorSpelling::Less,
                BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
                BinaryOperator::Greater => OperatorSpelling::Greater,
                BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
                BinaryOperator::And | BinaryOperator::Or => return Some(None),
                _ => return None,
            };
            if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                machine.symbol,
                expression,
                spelling,
                &[left, right],
            ) {
                return None;
            }
            // Only already-admitted builtin arithmetic inherits its operand
            // carrier. Literal types remain wildcard inputs to the owner.
            if matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
            ) {
                if left.zip(right).is_some_and(|(left, right)| {
                    program.primitive_type_reference(left)
                        != program.primitive_type_reference(right)
                }) {
                    return None;
                }
                Some(left.or(right))
            } else {
                Some(None)
            }
        }
        _ => None,
    }
}

/// Bind each Exact non-polynomial term admitted by this ranking query.
/// The general strict arithmetic engine does not infer executable division
/// or shifting from a token. Anonymous rational subtrees still fold as
/// rationals; each landed quotient, remainder, or shift retains both
/// operands for state transport. This is meaning, not formation: the range
/// owner still checks every operation before using the endpoint, including
/// an overflowing intermediate quotient or an out-of-width shift count.
pub(super) fn install_nonpolynomial_terms(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    engine: &mut super::Engine<'_>,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<()> {
    if depth >= 128 {
        return None;
    }
    if super::super::proof_integer::anonymous_integer_value(program, expression).is_some() {
        return Some(());
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            install_nonpolynomial_terms(program, machine, state, engine, atomic.value, depth + 1)?;
        }
        ExpressionNode::Binary(binary) => {
            for operand in [binary.left, binary.right] {
                install_nonpolynomial_terms(program, machine, state, engine, operand, depth + 1)?;
            }
            if matches!(
                binary.operator,
                BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::BitwiseAnd
            ) {
                if let Some(carrier) = builtin(program, machine, state, expression, 0)? {
                    let primitive = super::exact_integer_parameter(program, carrier)?;
                    match binary.operator {
                        BinaryOperator::Divide | BinaryOperator::Modulo => {
                            engine.bind_strict_integer_division(expression)?
                        }
                        // The count's defined range is the shifted
                        // carrier's width (the F8 ruling), so the term mints
                        // with it rather than re-deriving a bound.
                        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => engine
                            .bind_strict_integer_shift(
                                expression,
                                crate::proof_contracts::arithmetic_domains::integer_bit_width(
                                    primitive,
                                )? as u32,
                            )?,
                        // `&` is total inside the shared carrier, so the
                        // term keeps the carrier's shape -- width and
                        // signedness -- rather than a defined-range bound.
                        BinaryOperator::BitwiseAnd => engine.bind_strict_integer_bitwise_and(
                            expression,
                            crate::proof_contracts::arithmetic_domains::integer_bit_width(
                                primitive,
                            )? as u32,
                            primitive.is_signed_integer(),
                        )?,
                        _ => return None,
                    }
                } else if binary.operator != BinaryOperator::Modulo {
                    return None;
                }
                // Natural coordinates such as a slice length have no machine
                // carrier here. Their constant-modulus normalization remains
                // with the existing arithmetic engine, not a fabricated width.
            }
        }
        _ => {}
    }
    Some(())
}
