//! Distinguish a resolved operator declaration namespace from a storage receiver.

/// A named operator's complete declaration namespace is not a storage read.
/// Retained value identities never acquire namespace status from spelling.
pub(crate) fn is_named_operator_namespace(
    program: &dyn crate::CheckingInput,
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
        target_symbol,
        Some(namespace),
        target,
        argument_count,
        false,
    )
    .is_some_and(|operator| {
        // A settled result overload already owns its declaration. Authenticating
        // its namespace must not reselect the overloaded family by spelling and
        // mistake ordinary result dispatch for a data receiver.
        program.operator_parameters(operator).len() == argument_count
            && program
                .operator_path_members(operator.name)
                .split_last()
                .is_some_and(|(member, prefix)| {
                    member.as_str() == target
                        && prefix.len() == namespace.len()
                        && prefix
                            .iter()
                            .zip(namespace)
                            .all(|(expected, actual)| expected.as_str() == *actual)
                })
    })
}

pub(crate) fn expression_call_has_operator_namespace(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    let table = &program.expression_table;
    let Some(path) = table.name_path_node(call.receiver) else {
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
    let target_symbol = typed_trees::operator::resolve_named_expression_call(program, call)
        .map(|operator| operator.symbol)
        .unwrap_or(call.target_symbol);
    is_named_operator_namespace(
        program,
        &receiver_symbols,
        &namespace,
        target_symbol,
        call.target.as_str(),
        table.expression_handles(call.arguments).len(),
    )
}
