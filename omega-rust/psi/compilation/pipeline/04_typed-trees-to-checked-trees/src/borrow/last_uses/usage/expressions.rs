use checked_trees::expression::ExpressionHandle;

pub(super) fn expression_uses_local_name(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    local_name: &str,
) -> bool {
    match program.expression_table.expression(expression) {
        typed_trees::expression::ExpressionNode::SavedSubject(saved) => program
            .saved_value_source(saved)
            .is_some_and(|source| expression_uses_local_name(program, source, local_name)),
        typed_trees::expression::ExpressionNode::Match(dispatch) => {
            expression_uses_local_name(program, dispatch.subject, local_name)
                || program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        program
                            .expression_table
                            .match_pattern_operands(arm.pattern)
                            .any(|pattern| expression_uses_local_name(program, pattern, local_name))
                            || expression_uses_local_name(program, arm.value, local_name)
                    })
        }
        typed_trees::expression::ExpressionNode::Atomic(atomic) => {
            expression_uses_local_name(program, atomic.value, local_name)
        }
        typed_trees::expression::ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_uses_local_name(program, *value, local_name)),
        typed_trees::expression::ExpressionNode::Binary(binary) => {
            expression_uses_local_name(program, binary.left, local_name)
                || expression_uses_local_name(program, binary.right, local_name)
        }
        typed_trees::expression::ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && expression_uses_local_name(program, call.receiver, local_name))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses_local_name(program, *argument, local_name))
        }
        typed_trees::expression::ExpressionNode::Cast(cast) => {
            expression_uses_local_name(program, cast.value, local_name)
        }
        typed_trees::expression::ExpressionNode::Indexed(indexed) => {
            expression_uses_local_name(program, indexed.collection, local_name)
                || expression_uses_local_name(program, indexed.index, local_name)
        }
        typed_trees::expression::ExpressionNode::Range(range) => {
            (range.start.is_valid() && expression_uses_local_name(program, range.start, local_name))
                || (range.end.is_valid()
                    && expression_uses_local_name(program, range.end, local_name))
        }
        typed_trees::expression::ExpressionNode::Member(member) => {
            expression_uses_local_name(program, member.receiver, local_name)
        }
        typed_trees::expression::ExpressionNode::Borrow(inner_expression) => {
            expression_uses_local_name(program, inner_expression.target, local_name)
        }
        typed_trees::expression::ExpressionNode::Unary(unary) => {
            expression_uses_local_name(program, unary.operand, local_name)
        }
        typed_trees::expression::ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .first()
            .is_some_and(|member| member.as_str() == local_name),
        typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => program
            .expression_table
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| expression_uses_local_name(program, field.value, local_name)),
        typed_trees::expression::ExpressionNode::Boolean(_)
        | typed_trees::expression::ExpressionNode::Float(_)
        | typed_trees::expression::ExpressionNode::UndeterminedNaN(_)
        | typed_trees::expression::ExpressionNode::Integer(_)
        | typed_trees::expression::ExpressionNode::String(_)
        | typed_trees::expression::ExpressionNode::ZeroValue(_) => false,
    }
}
