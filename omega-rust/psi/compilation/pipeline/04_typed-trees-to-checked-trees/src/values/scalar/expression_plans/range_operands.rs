//! Retain endpoints for computed structural operands at expression occurrences.
//! Planning stores bindings only; selective evaluation remains with the consumer.
use super::{
    ExpressionNode, PrimitiveType, StatementNode, StatementPlanner, lower_return_expression,
    retain_subslice_endpoints,
};

pub(super) fn plan(planner: StatementPlanner<'_, '_>, statement: &StatementNode) {
    let StatementPlanner {
        program,
        operators,
        exact_integer_casts,
        state,
        parameters,
        scalar_parameters,
        parameter_types,
        statement_ordinal,
        locals,
        expressions,
        source_bindings,
        binding_symbols,
        ..
    } = planner;
    let table = &program.expression_table;
    let mut pending: Vec<_> = program
        .statement_table
        .expression_roots(statement)
        .into_iter()
        .map(|(_, root)| root)
        .filter(|root| root.is_valid())
        .collect();
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !table.expression_is_valid(expression) || visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        let endpoints = super::super::subslice_endpoints::subslice_endpoints(
            program,
            operators,
            expression,
            checked_trees::CheckedSubsliceSite::Expression { expression },
            |endpoint| {
                lower_return_expression(
                    program,
                    operators,
                    endpoint,
                    scalar_parameters,
                    parameters,
                    parameter_types,
                    locals,
                    PrimitiveType::U64,
                    exact_integer_casts,
                )
            },
        );
        retain_subslice_endpoints(
            endpoints,
            state.symbol,
            statement_ordinal,
            scalar_parameters,
            locals,
            expressions,
            source_bindings,
            binding_symbols,
        );
        match table.expression(expression) {
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Indexed(indexed) => pending.extend([indexed.collection, indexed.index]),
            ExpressionNode::Range(range) => pending.extend(
                [range.start, range.end]
                    .into_iter()
                    .filter(|child| child.is_valid()),
            ),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    pending.push(call.receiver);
                }
                pending.extend_from_slice(table.expression_handles(call.arguments));
            }
            ExpressionNode::ArrayLiteral(elements) => {
                pending.extend_from_slice(table.expression_handles(*elements))
            }
            ExpressionNode::StructLiteral(literal) => pending.extend(
                table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::Atomic(atomic) => {
                pending.push(atomic.value);
                if atomic.result.is_valid() {
                    pending.push(atomic.result);
                }
            }
            ExpressionNode::Match(dispatch) => {
                pending.push(dispatch.subject);
                for arm in table.match_arms(dispatch.arms) {
                    if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
}
