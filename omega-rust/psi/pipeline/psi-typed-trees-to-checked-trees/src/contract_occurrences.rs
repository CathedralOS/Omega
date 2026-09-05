use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// Return each structural value occurrence referenced by one contract fact.
///
/// A member or indexed expression is one complete place. Runtime index
/// expressions are additional occurrences because their evaluated values
/// select which place the fact describes and must therefore participate in
/// revision invalidation too.
pub(crate) fn fact_referenced_occurrences(
    program: &psi_typed_trees::TypedTrees,
    fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
) -> Vec<ExpressionHandle> {
    let mut occurrences = Vec::new();
    match program.proof_facts.get(fact) {
        psi_typed_trees::domain::ProofFact::Expression(expression) => {
            append_expression_occurrences(program, *expression, &mut occurrences);
        }
        psi_typed_trees::domain::ProofFact::Membership(membership) => {
            append_expression_occurrences(program, membership.value, &mut occurrences);
        }
        psi_typed_trees::domain::ProofFact::Proposition(application) => {
            for argument in program
                .expression_table
                .expression_handles(application.arguments)
            {
                append_expression_occurrences(program, *argument, &mut occurrences);
            }
        }
    }
    occurrences.sort_by_key(|expression| expression.arena_index());
    occurrences.dedup();
    occurrences
}

pub(crate) fn append_expression_occurrences(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    occurrences: &mut Vec<ExpressionHandle>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member)
            if matches!(
                program.expression_table.expression(member.receiver),
                ExpressionNode::Call(_)
            ) =>
        {
            append_expression_occurrences(program, member.receiver, occurrences);
        }
        ExpressionNode::Name(_) => occurrences.push(expression),
        ExpressionNode::Member(member) => {
            occurrences.push(expression);
            append_selector_occurrences(program, member.receiver, occurrences);
        }
        ExpressionNode::Indexed(indexed) => {
            occurrences.push(expression);
            append_selector_occurrences(program, indexed.collection, occurrences);
            append_expression_occurrences(program, indexed.index, occurrences);
        }
        ExpressionNode::Borrow(inner) => {
            append_expression_occurrences(program, inner.target, occurrences);
        }
        ExpressionNode::Atomic(atomic) => {
            append_expression_occurrences(program, atomic.value, occurrences);
            append_expression_occurrences(program, atomic.result, occurrences);
        }
        ExpressionNode::Binary(binary) => {
            append_expression_occurrences(program, binary.left, occurrences);
            append_expression_occurrences(program, binary.right, occurrences);
        }
        ExpressionNode::Cast(cast) => {
            append_expression_occurrences(program, cast.value, occurrences);
        }
        ExpressionNode::Call(call) => {
            append_expression_occurrences(program, call.receiver, occurrences);
            for argument in program.expression_table.expression_handles(call.arguments) {
                append_expression_occurrences(program, *argument, occurrences);
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                append_expression_occurrences(program, *value, occurrences);
            }
        }
        ExpressionNode::Range(range) => {
            append_expression_occurrences(program, range.start, occurrences);
            append_expression_occurrences(program, range.end, occurrences);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                append_expression_occurrences(program, field.value, occurrences);
            }
        }
        ExpressionNode::Unary(unary) => {
            append_expression_occurrences(program, unary.operand, occurrences);
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// A selected field remains one complete storage dependency, but selectors
/// beneath its receiver are independent values that can redirect the read.
fn append_selector_occurrences(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    occurrences: &mut Vec<ExpressionHandle>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            append_selector_occurrences(program, member.receiver, occurrences)
        }
        ExpressionNode::Indexed(indexed) => {
            append_selector_occurrences(program, indexed.collection, occurrences);
            append_expression_occurrences(program, indexed.index, occurrences);
        }
        ExpressionNode::Borrow(borrow) => {
            append_selector_occurrences(program, borrow.target, occurrences)
        }
        _ => {}
    }
}
