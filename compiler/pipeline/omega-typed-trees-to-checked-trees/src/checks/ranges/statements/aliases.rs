use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};

use super::super::facts::RangeFacts;

pub(super) fn seed_local_alias_facts(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    value: ExpressionHandle,
    local_name: Option<&str>,
) {
    if !value.is_valid() {
        return;
    }
    let Some(local_name) = local_name else {
        return;
    };
    let Some(source) = alias_source_label(program, value) else {
        return;
    };

    facts.alias_collection(&source, local_name);
    facts.alias_index(&source, local_name);
}

fn alias_source_label(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call)
            if matches!(call.target.as_str(), "as_slice" | "as_mut_slice") =>
        {
            Some(program.expression_table.display_name(call.receiver))
        }
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            Some(program.expression_table.display_name(expression))
        }
        _ => None,
    }
}
