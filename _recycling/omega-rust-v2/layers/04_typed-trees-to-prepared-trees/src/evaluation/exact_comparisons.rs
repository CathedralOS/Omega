use omega_prepared::PreparedComparisons;
use omega_resolved::TreeNodeId;
use omega_syntax::NodeKind;
use omega_typed::{NodeSemantics, TypedOperation, TypedTree, TypedTrees, ValueType};
use std::collections::HashMap;

/// Typing already normalized the operands. Folding may skip only anonymous
/// arithmetic, since calls, bindings and landed expressions have their own meaning.
pub(crate) fn value(
    tree: &TypedTree<'_>,
    operation: &TypedOperation,
) -> Option<bool> {
    let [
        ValueType::Exact { value: left, .. },
        ValueType::Exact { value: right, .. },
    ] = operation.operands.as_slice()
    else {
        return None;
    };
    let node = operation.node.node;
    let file = tree.resolved().file;
    if operation.result != ValueType::Bool || file.nodes[node].kind != NodeKind::Binary {
        return None;
    }
    for (child, syntax) in file.nodes.iter().enumerate().take(file.nodes[node].end).skip(node + 1) {
        if !matches!(syntax.kind, NodeKind::Literal | NodeKind::Paren | NodeKind::Binary | NodeKind::Unary)
            || !tree.semantics_at(child).all(|record| match record {
                NodeSemantics::Operation(op) => matches!(op.result, ValueType::Exact { .. }),
                NodeSemantics::Expression(e) => e.static_argument_slot.is_none() && matches!(e.ty, ValueType::Exact { .. }),
                _ => false,
            })
        {
            return None;
        }
    }
    Some(match operation.token.as_str() {
        "==" => left == right,
        "!=" => left != right,
        "<" => left < right,
        "<=" => left <= right,
        ">" => left > right,
        ">=" => left >= right,
        _ => return None,
    })
}

pub(crate) fn freeze(typed: &TypedTrees<'_>) -> PreparedComparisons {
    fn in_forest(typed: &TypedTrees<'_>) -> HashMap<TreeNodeId, bool> {
        let trees: Vec<_> = typed.trees().collect();
        typed
            .operations
            .iter()
            .filter_map(|operation| {
                let tree = trees.get(operation.node.file)?;
                Some((operation.node, value(tree, operation)?))
            })
            .collect()
    }

    PreparedComparisons {
        package: in_forest(typed),
        library: typed.library.map(in_forest).unwrap_or_default(),
    }
}
