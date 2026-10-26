//! Executable-expression membership, without result metadata edges.

/// These membership sets are never iterated for authored output order.
type HashSet<T> = std::collections::HashSet<T, symbols::BuildSymbolHasher>;
use typed_trees::expression::ExpressionNode;

/// Whether the containment walk from `root` reaches `target`.
pub fn expression_contains(
    program: &dyn crate::CheckingInput,
    root: typed_trees::expression::ExpressionHandle,
    target: typed_trees::expression::ExpressionHandle,
) -> bool {
    // Exact identity and leaf membership need no traversal inventory. Keep
    // the shared walk unchanged for callers collecting the complete DAG.
    if !root.is_valid() || !target.is_valid() {
        return false;
    }
    if root == target {
        return true;
    }
    if matches!(
        program.expression_table.expression(root),
        typed_trees::literal_expression_pattern!()
    ) {
        return false;
    }
    reaches(program, root, target, &mut HashSet::default())
}

/// Every expression the containment walk reaches from `root`.
///
/// `expression_contains(program, root, target)` holds exactly when this set
/// contains `target`: the walk records each expression it enters before
/// comparing it, and an address no expression occupies never ends the walk
/// early, so the recorded set is the whole reachable set. A caller asking
/// the same containment question of many targets indexes this once instead
/// of rewalking `root` per target.
pub fn reachable_expressions(
    program: &dyn crate::CheckingInput,
    root: typed_trees::expression::ExpressionHandle,
) -> HashSet<typed_trees::expression::ExpressionHandle> {
    let mut reached = HashSet::default();
    // Arena index zero is the invalid address; `is_valid` rejects it below,
    // so no recorded expression can equal it.
    reaches(
        program,
        root,
        typed_trees::expression::ExpressionHandle::invalid(),
        &mut reached,
    );
    reached
}

/// The shared walk: a membership set, never an ordered history, so an
/// expression reached twice through different parents is entered once.
fn reaches(
    program: &dyn crate::CheckingInput,
    root: typed_trees::expression::ExpressionHandle,
    target: typed_trees::expression::ExpressionHandle,
    visited: &mut HashSet<typed_trees::expression::ExpressionHandle>,
) -> bool {
    if !root.is_valid() || !visited.insert(root) {
        return false;
    }
    if root == target {
        return true;
    }
    match program.expression_table.expression(root) {
        ExpressionNode::Match(dispatch) => {
            reaches(program, dispatch.subject, target, visited)
                || program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        program
                            .expression_table
                            .match_pattern_operands(arm.pattern)
                            .any(|pattern| reaches(program, pattern, target, visited))
                            || reaches(program, arm.value, target, visited)
                    })
        }
        ExpressionNode::Atomic(atomic) => reaches(program, atomic.value, target, visited),
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|child| reaches(program, *child, target, visited)),
        ExpressionNode::Binary(binary) => {
            reaches(program, binary.left, target, visited)
                || reaches(program, binary.right, target, visited)
        }
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && reaches(program, call.receiver, target, visited))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|child| reaches(program, *child, target, visited))
        }
        ExpressionNode::Cast(cast) => reaches(program, cast.value, target, visited),
        ExpressionNode::Indexed(indexed) => {
            reaches(program, indexed.collection, target, visited)
                || reaches(program, indexed.index, target, visited)
        }
        ExpressionNode::Member(member) => reaches(program, member.receiver, target, visited),
        ExpressionNode::Borrow(inner) => reaches(program, inner.target, target, visited),
        ExpressionNode::Range(range) => {
            reaches(program, range.start, target, visited)
                || reaches(program, range.end, target, visited)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| reaches(program, field.value, target, visited)),
        ExpressionNode::Unary(unary) => reaches(program, unary.operand, target, visited),
        ExpressionNode::SavedSubject(_) | typed_trees::literal_expression_pattern!() => false,
    }
}
