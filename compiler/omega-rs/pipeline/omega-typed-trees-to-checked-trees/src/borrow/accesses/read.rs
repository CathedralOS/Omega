use super::*;

pub(super) fn collect_read_accesses(
    collection: &mut BorrowAccessCollection<'_>,
    expression: ExpressionHandle,
) {
    match collection.program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => collect_read_accesses(collection, atomic.value),
        ExpressionNode::ArrayLiteral(values) => {
            for value in collection
                .program
                .expression_table
                .expression_handles(*values)
            {
                collect_read_accesses(collection, *value);
            }
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                collect_read_accesses(collection, range.start);
            }
            if range.end.is_valid() {
                collect_read_accesses(collection, range.end);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_read_accesses(collection, binary.left);
            collect_read_accesses(collection, binary.right);
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_read_accesses(collection, call.receiver);
            }

            for argument in collection
                .program
                .expression_table
                .expression_handles(call.arguments)
            {
                collect_read_accesses(collection, *argument);
            }
        }
        ExpressionNode::Cast(cast) => collect_read_accesses(collection, cast.value),
        ExpressionNode::Indexed(indexed) => {
            append_read_access(collection, expression);

            collect_read_accesses(collection, indexed.index);
        }
        ExpressionNode::Member(member) => {
            if !append_read_access(collection, expression) {
                collect_read_accesses(collection, member.receiver);
            }
        }
        ExpressionNode::Name(_) => {
            append_read_access(collection, expression);
        }
        ExpressionNode::Mutable(inner_expression) => {
            collect_read_accesses(collection, *inner_expression)
        }
        ExpressionNode::Unary(unary) => collect_read_accesses(collection, unary.operand),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in collection
                .program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_read_accesses(collection, field.value);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn append_read_access(
    collection: &mut BorrowAccessCollection<'_>,
    expression: ExpressionHandle,
) -> bool {
    let Some(access_place) = collection.borrow_access_place(expression) else {
        return false;
    };

    collection.append_argument_access(access_place, BorrowAccessKind::Read);
    true
}
