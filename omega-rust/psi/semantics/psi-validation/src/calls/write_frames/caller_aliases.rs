//! Caller-prefix alias closure for public write-frame demand.
//!
//! The state walker owns alias transfer. This leaf locates a query's statement
//! and closes its writes over the canonical storage paths and live local names
//! used by fact consumers. It never publishes prefix writes as call writes.

use super::{
    ExpressionHandle, ExpressionNode, FramePathPrecision, FramePlaceOrigin, Machine, StatementNode,
    SymbolHandle, TableCall, TopLevelSymbols, TypedTrees, append_place_suffix,
    rebase_local_alias_path, statement_value_expression_roots, type_is_caller_isolated_local,
    type_may_carry_write, walk_state_write_prefix,
};

pub(super) enum CallerWriteSite<'query> {
    Call(&'query TableCall),
    Statement(&'query StatementNode),
    Expression(ExpressionHandle),
}

/// Transient caller-prefix evidence for projecting a local reference into
/// another place representation. Coarse storage paths cannot acquire a field
/// or index suffix when the consumer transports a write through this origin.
pub struct LocalWriteOrigin {
    pub local_symbol: SymbolHandle,
    pub source_path: String,
    pub collection_coarse: bool,
}

pub(super) fn local_write_origins_before_statement(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    statement: &StatementNode,
) -> Option<Vec<LocalWriteOrigin>> {
    let aliases = caller_aliases_at_site(
        program,
        machine,
        symbols,
        CallerWriteSite::Statement(statement),
    )?;
    if aliases.is_empty() {
        return Some(Vec::new());
    }
    let state = program.machine_states(machine).iter().find(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|candidate| std::ptr::eq(statement, candidate))
    })?;
    aliases
        .into_iter()
        .map(|(name, origin)| {
            let local_symbol = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take_while(|candidate| !std::ptr::eq(statement, *candidate))
                .find_map(|candidate| match candidate {
                    StatementNode::LocalData(local) if local.name.as_str() == name => {
                        Some(local.symbol)
                    }
                    _ => None,
                })
                .filter(|symbol| {
                    symbol.is_valid()
                        && program.symbols.get(*symbol).parent == state.symbol
                        && matches!(
                            program.symbols.get(*symbol).kind,
                            psi_symbols::SymbolKind::Local
                        )
                })?;
            Some(LocalWriteOrigin {
                local_symbol,
                source_path: origin.path,
                collection_coarse: origin.precision == FramePathPrecision::CollectionCoarse,
            })
        })
        .collect()
}

pub(super) fn close_caller_aliases(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    site: CallerWriteSite<'_>,
    written: Vec<String>,
) -> Option<Vec<String>> {
    if written.is_empty() {
        return Some(written);
    }
    let aliases = caller_aliases_at_site(program, machine, symbols, site)?;
    let canonical = written
        .iter()
        .map(|path| rebase_local_alias_path(path, &aliases))
        .collect::<Vec<_>>();
    let mut closed = written;
    for path in canonical {
        if !closed.contains(&path) {
            closed.push(path.clone());
        }
        for (alias, origin) in &aliases {
            let spelling = if let Some(suffix) = place_suffix(&origin.path, &path) {
                match origin.precision {
                    FramePathPrecision::Exact => append_place_suffix(alias, suffix),
                    FramePathPrecision::CollectionCoarse => alias.clone(),
                }
            } else if place_suffix(&path, &origin.path).is_some() {
                alias.clone()
            } else {
                continue;
            };
            if !closed.contains(&spelling) {
                closed.push(spelling);
            }
        }
    }
    Some(closed)
}

fn caller_aliases_at_site(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    site: CallerWriteSite<'_>,
) -> Option<Vec<(String, FramePlaceOrigin)>> {
    let may_declare_alias = |statement: &StatementNode| {
        matches!(statement, StatementNode::LocalData(local)
            if type_may_carry_write(program, local.type_reference)
                && !type_is_caller_isolated_local(program, local.type_reference))
    };
    if !program.machine_states(machine).iter().any(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(may_declare_alias)
    }) {
        return Some(Vec::new());
    }
    let mut owner = None;
    for state in program.machine_states(machine) {
        let statements = program.statement_table.statements(state.statement_nodes);
        for (index, statement) in statements.iter().enumerate() {
            let matches = match site {
                CallerWriteSite::Call(call) => {
                    matches!(statement, StatementNode::Call(candidate) if std::ptr::eq(call, candidate))
                }
                CallerWriteSite::Statement(candidate) => std::ptr::eq(statement, candidate),
                CallerWriteSite::Expression(expression) => {
                    statement_value_expression_roots(program, statement)
                        .into_iter()
                        .any(|root| contains_expression(program, root, expression))
                }
            };
            if matches {
                if owner.is_some() {
                    return None;
                }
                owner = Some((
                    state,
                    statement,
                    statements[..index].iter().any(may_declare_alias),
                ));
            }
        }
    }
    let (state, statement, has_aliases) = owner?;
    if !has_aliases {
        return Some(Vec::new());
    }
    let prefix = walk_state_write_prefix(
        program,
        machine,
        state,
        symbols,
        &mut Vec::new(),
        &mut Vec::new(),
        Some(statement),
    )?;
    Some(prefix.aliases)
}

fn place_suffix<'path>(root: &str, path: &'path str) -> Option<&'path str> {
    let suffix = path.strip_prefix(root)?;
    (suffix.is_empty() || suffix.starts_with('.')).then_some(suffix)
}

fn contains_expression(
    program: &TypedTrees,
    root: ExpressionHandle,
    target: ExpressionHandle,
) -> bool {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if expression == target {
            return true;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Atomic(atomic) => pending.push(atomic.value),
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Call(call) => {
                pending.push(call.receiver);
                pending.extend(program.expression_table.expression_handles(call.arguments));
            }
            ExpressionNode::Indexed(indexed) => pending.extend([indexed.collection, indexed.index]),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::ArrayLiteral(elements) => {
                pending.extend(program.expression_table.expression_handles(*elements))
            }
            ExpressionNode::StructLiteral(literal) => pending.extend(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::Range(range) => pending.extend([range.start, range.end]),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
    false
}
