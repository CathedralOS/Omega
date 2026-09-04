use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::statement::{StatementNode, TableLocalData};

enum ImmutableLocalLookup<'program> {
    Missing,
    Found(&'program TableLocalData),
    Invalid,
}

/// Normalize an integer-bound expression through a finite chain of immutable
/// local copies.
///
/// The terminal leaf is either the original integer literal or one exact
/// symbol-backed bare name, such as a machine/state parameter. Mutable,
/// ambiguous, cyclic, qualified, and computed aliases remain unknown. Typed
/// local-name expressions currently retain no symbol, so an unresolved bare
/// spelling is accepted only when it names one unique immutable local in the
/// complete typed program.
pub fn normalize_immutable_integer_bound_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    normalize_bound(program, expression, &mut Vec::new())
}

/// Normalize an integer literal or finite immutable local-copy chain to one
/// exact host index. Symbolic parameter leaves and every unsupported alias
/// shape remain unknown.
pub fn normalize_immutable_integer_bound_to_usize(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<usize> {
    let expression = normalize_immutable_integer_bound_expression(program, expression)?;
    program
        .expression_table
        .constant_integer_value(expression)
        .and_then(|value| usize::try_from(value).ok())
}

fn normalize_bound(
    program: &TypedTrees,
    expression: ExpressionHandle,
    seen_aliases: &mut Vec<SymbolHandle>,
) -> Option<ExpressionHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => Some(expression),
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if members.len() != 1 {
                return None;
            }
            if path.symbol.is_valid() && path.head_symbol == path.symbol {
                match immutable_local_by_symbol(program, path.symbol) {
                    ImmutableLocalLookup::Missing => Some(expression),
                    ImmutableLocalLookup::Found(local) => {
                        normalize_local(program, local, seen_aliases)
                    }
                    ImmutableLocalLookup::Invalid => None,
                }
            } else {
                match immutable_local_by_name(program, members[0].as_str()) {
                    ImmutableLocalLookup::Found(local) => {
                        normalize_local(program, local, seen_aliases)
                    }
                    ImmutableLocalLookup::Missing | ImmutableLocalLookup::Invalid => None,
                }
            }
        }
        _ => None,
    }
}

fn normalize_local(
    program: &TypedTrees,
    local: &TableLocalData,
    seen_aliases: &mut Vec<SymbolHandle>,
) -> Option<ExpressionHandle> {
    if !local.symbol.is_valid() || seen_aliases.contains(&local.symbol) {
        return None;
    }
    seen_aliases.push(local.symbol);
    let normalized = normalize_bound(program, local.initial_value, seen_aliases);
    seen_aliases.pop();
    normalized
}

fn immutable_local_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> ImmutableLocalLookup<'_> {
    immutable_local(program, |local| local.symbol == symbol)
}

fn immutable_local_by_name<'program>(
    program: &'program TypedTrees,
    name: &str,
) -> ImmutableLocalLookup<'program> {
    immutable_local(program, |local| local.name.as_str() == name)
}

fn immutable_local(
    program: &TypedTrees,
    matches: impl Fn(&TableLocalData) -> bool,
) -> ImmutableLocalLookup<'_> {
    let mut matching = None;
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                if !matches(local) {
                    continue;
                }
                if matching.is_some() || local.is_mutable {
                    return ImmutableLocalLookup::Invalid;
                }
                matching = Some(local);
            }
        }
    }
    matching.map_or(ImmutableLocalLookup::Missing, ImmutableLocalLookup::Found)
}
