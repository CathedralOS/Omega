use crate::flow::expressions;
use checked_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::{SubtreeEdges, SubtreeVisit};

pub(super) fn expression_uses_local_name(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    local_name: &str,
) -> bool {
    program.subtree_any(
        expression,
        SubtreeEdges::SAVED_SOURCES,
        |_, node| match node {
            ExpressionNode::Name(path)
                if expressions(program)
                    .name_path_members(path.members)
                    .first()
                    .is_some_and(|member| member.as_str() == local_name) =>
            {
                SubtreeVisit::Found
            }
            _ => SubtreeVisit::Descend,
        },
    )
}
