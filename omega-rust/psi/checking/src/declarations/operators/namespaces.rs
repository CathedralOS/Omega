//! Distinguish a resolved operator declaration namespace from a storage receiver.

use typed_trees::TypedTrees;

/// A named operator's complete declaration namespace is not a storage read.
/// Retained value identities never acquire namespace status from spelling.
pub(crate) fn is_named_operator_namespace(
    program: &TypedTrees,
    receiver_symbols: &[symbols::SymbolHandle],
    namespace: &[&str],
    target_symbol: symbols::SymbolHandle,
    target: &str,
    argument_count: usize,
) -> bool {
    if namespace.is_empty()
        || receiver_symbols.iter().any(|symbol| {
            symbol.is_valid()
                && !matches!(
                    program.symbols.get(*symbol).kind,
                    symbols::SymbolKind::Module
                        | symbols::SymbolKind::BuiltinType
                        | symbols::SymbolKind::Data
                        | symbols::SymbolKind::Domain
                        | symbols::SymbolKind::Machine
                        | symbols::SymbolKind::Trait
                )
        })
    {
        return false;
    }
    typed_trees::operator::resolve_named_call(
        program,
        symbols::SymbolHandle::invalid(),
        Some(namespace),
        target,
        argument_count,
        false,
    )
    .is_some_and(|operator| !target_symbol.is_valid() || target_symbol == operator.symbol)
}

pub(crate) fn expression_call_has_operator_namespace(
    program: &TypedTrees,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    let table = &program.expression_table;
    let typed_trees::expression::ExpressionNode::Name(path) = table.expression(call.receiver)
    else {
        return false;
    };
    let receiver_symbols: Vec<_> = [path.head_symbol, path.symbol]
        .into_iter()
        .chain(
            table
                .name_path_member_symbols(path.member_symbols)
                .iter()
                .copied(),
        )
        .collect();
    let namespace: Vec<_> = table
        .name_path_members(path.members)
        .iter()
        .map(|member| member.as_str())
        .collect();
    is_named_operator_namespace(
        program,
        &receiver_symbols,
        &namespace,
        call.target_symbol,
        call.target.as_str(),
        table.expression_handles(call.arguments).len(),
    )
}
