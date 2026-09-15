use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TableLocalData};
use typed_trees::types::PrimitiveType;

/// One immutable integer boundary shifted by a compile-time constant: the
/// mathematical value `symbol + offset`, valid only under Exact arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImmutableIntegerBoundOffset {
    pub symbol: SymbolHandle,
    pub offset: i64,
}

/// Immutable integer boundary normalization preserves either an exact literal,
/// one immutable binding's identity, or one resolved immutable symbol leaf.
///
/// A constant offset requires Exact arithmetic on the immediate operand because
/// exact-domain `+`/`-` is a proof obligation discharged before checking
/// (decision 17, `arithmetic_domains.rs`); with the selector already passing
/// slice-range validation, its runtime value is the mathematical
/// `symbol + offset`. Wrapping, Saturating, and Trapping operands may wrap or
/// clamp and therefore remain unknown.
#[cfg(test)]
mod tests;

enum LocalLookup<'program> {
    Missing,
    Found(&'program TableLocalData),
    Invalid,
}

enum NormalizedBound {
    Expression(ExpressionHandle),
    LocalValue(SymbolHandle),
    MutableValue,
}

/// Normalize an integer-bound expression through a finite chain of immutable
/// local copies.
///
/// The terminal leaf is either the original integer literal or one exact
/// symbol-backed bare name, such as a machine/state parameter. Mutable,
/// ambiguous, cyclic, qualified, and computed aliases remain unknown. A bare
/// local name without a retained symbol is accepted only when it names one
/// unique immutable local in the complete typed program.
pub fn normalize_immutable_integer_bound_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    match normalize_bound(program, expression, &mut Vec::new())? {
        NormalizedBound::Expression(expression) => Some(expression),
        NormalizedBound::LocalValue(_) | NormalizedBound::MutableValue => None,
    }
}

/// Retain the identity of one immutable integer binding whose initializer is
/// computed or reads a mutable value, including through immutable copies.
/// This establishes shared value identity, not the initializer's value or
/// equivalence to a separately evaluated computation.
/// Static-index callers continue to use the expression/usize normalizers.
pub fn immutable_integer_bound_value_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    match normalize_bound(program, expression, &mut Vec::new())? {
        NormalizedBound::LocalValue(symbol) => Some(symbol),
        NormalizedBound::Expression(_) | NormalizedBound::MutableValue => None,
    }
}

pub fn immutable_integer_bound_symbol_offset(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<ImmutableIntegerBoundOffset> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    let (base, offset) = match binary.operator {
        typed_trees::expression::BinaryOperator::Add => {
            let left_value = program.expression_table.constant_integer_value(binary.left);
            let right_value = program
                .expression_table
                .constant_integer_value(binary.right);
            match (left_value, right_value) {
                (Some(k), None) => (binary.right, k),
                (None, Some(k)) => (binary.left, k),
                _ => return None,
            }
        }
        typed_trees::expression::BinaryOperator::Subtract => {
            let k = program
                .expression_table
                .constant_integer_value(binary.right)?;
            if program
                .expression_table
                .constant_integer_value(binary.left)
                .is_some()
            {
                return None;
            }
            (binary.left, k.checked_neg()?)
        }
        _ => return None,
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(base) else {
        return None;
    };
    let members = program.expression_table.name_path_members(path.members);
    if members.len() != 1 {
        return None;
    }

    let type_reference = if path.symbol.is_valid() && path.head_symbol == path.symbol {
        match local_by_symbol(program, path.symbol) {
            LocalLookup::Found(local) => local.type_reference,
            LocalLookup::Missing => parameter_by_symbol(program, path.symbol)?.type_reference,
            LocalLookup::Invalid => return None,
        }
    } else {
        match local_by_name(program, members[0].as_str()) {
            LocalLookup::Found(local) => local.type_reference,
            LocalLookup::Missing | LocalLookup::Invalid => return None,
        }
    };
    let primitive = program
        .type_reference_table
        .primitive_type(type_reference)?;
    if !matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) || program.arithmetic_domain_for_type_reference(type_reference)
        != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }

    let symbol = match normalize_bound(program, base, &mut Vec::new())? {
        NormalizedBound::LocalValue(symbol) => symbol,
        NormalizedBound::Expression(expression) => {
            let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
                return None;
            };
            let members = program.expression_table.name_path_members(path.members);
            (members.len() == 1 && path.symbol.is_valid() && path.head_symbol == path.symbol)
                .then_some(path.symbol)?
        }
        NormalizedBound::MutableValue => return None,
    };
    Some(ImmutableIntegerBoundOffset { symbol, offset })
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
) -> Option<NormalizedBound> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => Some(NormalizedBound::Expression(expression)),
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            if members.len() != 1 {
                return None;
            }
            if path.symbol.is_valid() && path.head_symbol == path.symbol {
                match local_by_symbol(program, path.symbol) {
                    LocalLookup::Missing => {
                        if parameter_mutability(program, path.symbol)? {
                            Some(NormalizedBound::MutableValue)
                        } else {
                            Some(NormalizedBound::Expression(expression))
                        }
                    }
                    LocalLookup::Found(local) => normalize_local(program, local, seen_aliases),
                    LocalLookup::Invalid => None,
                }
            } else {
                match local_by_name(program, members[0].as_str()) {
                    LocalLookup::Found(local) => normalize_local(program, local, seen_aliases),
                    LocalLookup::Missing | LocalLookup::Invalid => None,
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
) -> Option<NormalizedBound> {
    if !local.symbol.is_valid() || seen_aliases.contains(&local.symbol) {
        return None;
    }
    if local.is_mutable {
        return Some(NormalizedBound::MutableValue);
    }
    seen_aliases.push(local.symbol);
    let normalized = match program.expression_table.expression(local.initial_value) {
        ExpressionNode::Integer(_) | ExpressionNode::Name(_) => {
            normalize_bound(program, local.initial_value, seen_aliases).map(|bound| match bound {
                // The local stores a value, not a retargetable alias to its source.
                NormalizedBound::MutableValue => NormalizedBound::LocalValue(local.symbol),
                bound => bound,
            })
        }
        _ if local.initial_value.is_valid() => Some(NormalizedBound::LocalValue(local.symbol)),
        _ => None,
    };
    seen_aliases.pop();
    normalized
}

fn local_by_symbol(program: &TypedTrees, symbol: SymbolHandle) -> LocalLookup<'_> {
    unique_local(program, |local| local.symbol == symbol)
}

fn local_by_name<'program>(program: &'program TypedTrees, name: &str) -> LocalLookup<'program> {
    unique_local(program, |local| local.name.as_str() == name)
}

fn unique_local(
    program: &TypedTrees,
    matches: impl Fn(&TableLocalData) -> bool,
) -> LocalLookup<'_> {
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
                if matching.is_some() {
                    return LocalLookup::Invalid;
                }
                matching = Some(local);
            }
        }
    }
    matching.map_or(LocalLookup::Missing, LocalLookup::Found)
}

fn parameter_mutability(program: &TypedTrees, symbol: SymbolHandle) -> Option<bool> {
    let mut matching = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .flat_map(|state| program.state_parameters(state))
        .filter(|parameter| parameter.symbol == symbol);
    let is_mutable = matching
        .next()
        .is_some_and(|parameter| parameter.is_mutable);
    matching.next().is_none().then_some(is_mutable)
}

fn parameter_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::signature::StateParameter> {
    let mut matching = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .flat_map(|state| program.state_parameters(state))
        .filter(|parameter| parameter.symbol == symbol);
    let parameter = matching.next()?;
    matching.next().is_none().then_some(parameter)
}
