use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::ExpressionNode;

/// Bind semantic `as ... in Domain` sites to declaration identity. Validation
/// owns the diagnostic policy; this pass only publishes deterministic
/// identities so no checked consumer re-resolves a user spelling.
pub(crate) fn normalize_qualification_casts(program: &mut TypedTrees) {
    let updates = program
        .expression_table
        .expression_entries()
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            let [name] = program
                .expression_table
                .name_path_members(cast.semantic_domain)
            else {
                return None;
            };
            let matches = program
                .domain_definitions()
                .iter()
                .filter(|domain| {
                    same_semantic_name(domain.name.as_str(), name.as_str())
                        && program.normalized_type_identity(domain.target_type)
                            == program.normalized_type_identity(cast.target_type)
                })
                .collect::<Vec<_>>();
            let [domain] = matches.as_slice() else {
                return Some((handle, SymbolHandle::invalid()));
            };
            Some((handle, domain.symbol))
        })
        .collect::<Vec<_>>();

    for (handle, domain) in updates {
        let ExpressionNode::Cast(cast) = program.expression_table.expression_mut(handle) else {
            continue;
        };
        cast.semantic_domain_symbol = domain;
    }
}

fn same_semantic_name(left: &str, right: &str) -> bool {
    left == right
        || (!left.contains("::") && right.rsplit("::").next().is_some_and(|leaf| leaf == left))
        || (!right.contains("::") && left.rsplit("::").next().is_some_and(|leaf| leaf == right))
}
