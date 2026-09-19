//! The existing declaration/custody owner, not token spelling, admits arithmetic.
use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, Machine, State, TypedTrees, fields, lengths,
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

/// Bind only the exact integer divisions admitted by this ranking query. The
/// general strict arithmetic engine deliberately does not infer executable
/// division from a token. Anonymous rational subtrees still fold as rationals;
/// each landed quotient retains both operands for simultaneous state transport.
/// This is meaning, not formation: the range owner still checks every operation
/// before using the endpoint, including an overflowing intermediate quotient.
pub(super) fn install_integer_quotients(
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
            install_integer_quotients(program, machine, state, engine, atomic.value, depth + 1)?;
        }
        ExpressionNode::Binary(binary) => {
            for operand in [binary.left, binary.right] {
                install_integer_quotients(program, machine, state, engine, operand, depth + 1)?;
            }
            if binary.operator == BinaryOperator::Divide {
                let carrier = builtin(program, machine, state, expression, 0)??;
                super::exact_integer_parameter(program, carrier)?;
                engine.bind_strict_integer_quotient(expression)?;
            }
        }
        _ => {}
    }
    Some(())
}
