//! The typed decrease rule shared by runtime and proof slice recursion.

use crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup;
use symbols::SymbolHandle;

use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceNode};

/// `slice_tail_strictly_decreases` with the bound index supplied by the
/// caller: the lookup scans the whole program once, and a termination pass
/// probes this predicate per guarded self-call, so each pass fills the cell
/// at most once instead of once per call site.
pub fn slice_tail_strictly_decreases<'p>(
    program: &'p dyn crate::CheckingInput,
    guard: ExpressionHandle,
    argument: ExpressionHandle,
    parameter: &StateParameter,
    bound_lookup: &mut Option<ImmutableBoundLookup<'p>>,
) -> bool {
    if !parameter.symbol.is_valid() || !parameter_is_slice(program, parameter) {
        return false;
    }
    let argument = program
        .expression_table
        .borrow_expression(argument)
        .map_or(argument, |borrow| borrow.target);
    let Some(indexed) = program.expression_table.indexed_node(argument) else {
        return false;
    };
    let Some(range) = program.expression_table.range_node(indexed.index) else {
        return false;
    };
    if !names_parameter(program, indexed.collection, parameter)
        || range.end.is_valid()
        || range.end_inclusive
    {
        return false;
    }
    // The bound index scans the whole program once; the cheap shape gates
    // above reject most selectors before it is needed.
    let Some(start) = tail_bound(
        program,
        bound_lookup.get_or_insert_with(|| ImmutableBoundLookup::new(program)),
        range.start,
    ) else {
        return false;
    };
    if !tail_bound_is_positive(program, start) {
        return false;
    }
    let guard = match program.expression_table.expression(guard) {
        ExpressionNode::Binary(binary)
            if binary.operator == BinaryOperator::Equal
                && matches!(
                    program.expression_table.expression(binary.right),
                    ExpressionNode::Boolean(true)
                ) =>
        {
            binary.left
        }
        _ => guard,
    };
    let Some(binary) = program.expression_table.binary_node(guard) else {
        return false;
    };
    let Some(length) = crate::length_member(program, binary.left) else {
        return false;
    };
    // `len > m` proves `len >= m + 1`, so it discharges `len >= start` when
    // `m + 1 >= start`; `len >= m` needs `m >= start` directly.
    let bonus = match binary.operator {
        BinaryOperator::Greater => 1,
        BinaryOperator::GreaterOrEqual => 0,
        _ => return false,
    };
    names_parameter(program, length.receiver, parameter)
        && tail_bound(
            program,
            bound_lookup.get_or_insert_with(|| ImmutableBoundLookup::new(program)),
            binary.right,
        )
        .is_some_and(|bound| bound_ordering_at_least(program, bound, start, bonus))
}

/// One immutable range bound as an ordering key: an exact literal value, or
/// one immutable binding's value identity shifted by a compile-time constant.
/// `Symbol` keeps *which* binding the bound reads, never a guessed value.
#[derive(Debug, Clone, Copy)]
enum TailBound {
    Literal(usize),
    Symbol { symbol: SymbolHandle, offset: i64 },
}

/// Normalize an integer-bound expression to an ordering key through the shared
/// immutable-integer-bound machinery. Every other shape -- mutable, ambiguous,
/// cyclic, qualified, or computed beyond a constant shift -- stays unknown.
fn tail_bound(
    program: &dyn crate::CheckingInput,
    bound_lookup: &crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup<'_>,
    expression: ExpressionHandle,
) -> Option<TailBound> {
    if let Some(value) =
        crate::normalize_immutable_integer_bound_to_usize(program, bound_lookup, expression)
    {
        return Some(TailBound::Literal(value));
    }
    if let Some(bound) =
        crate::immutable_integer_bound_symbol_offset(program, bound_lookup, expression)
    {
        return Some(TailBound::Symbol {
            symbol: bound.symbol,
            offset: bound.offset,
        });
    }
    if let Some(leaf) =
        crate::normalize_immutable_integer_bound_expression(program, bound_lookup, expression)
    {
        return match program.expression_table.expression(leaf) {
            ExpressionNode::Name(path) if crate::is_single_segment_name_path(program, path) => {
                Some(TailBound::Symbol {
                    symbol: path.symbol,
                    offset: 0,
                })
            }
            _ => None,
        };
    }
    crate::immutable_integer_bound_value_symbol(program, bound_lookup, expression)
        .map(|symbol| TailBound::Symbol { symbol, offset: 0 })
}

/// `bound + bonus >= start` over ordering keys. `bonus` is 1 when the guard is
/// `len > bound` (which proves `len >= bound + 1`), 0 for `len >= bound`.
fn bound_ordering_at_least(
    program: &dyn crate::CheckingInput,
    bound: TailBound,
    start: TailBound,
    bonus: i64,
) -> bool {
    match (bound, start) {
        (TailBound::Literal(bound), TailBound::Literal(start)) => {
            bound as i128 + bonus as i128 >= start as i128
        }
        (
            TailBound::Symbol {
                symbol: bound_symbol,
                offset: bound_offset,
            },
            TailBound::Symbol {
                symbol: start_symbol,
                offset: start_offset,
            },
        ) => {
            bound_symbol == start_symbol
                && bound_offset
                    .checked_add(bonus)
                    .is_some_and(|bound| bound >= start_offset)
        }
        // `symbol + offset >= literal` holds for every stored value when the
        // symbol's declared floor already clears `literal - offset - bonus`.
        (TailBound::Symbol { symbol, offset }, TailBound::Literal(start)) => {
            i64::try_from(start).ok().is_some_and(|start| {
                bound_symbol_floor(program, symbol)
                    .and_then(|floor| floor.checked_add(offset))
                    .and_then(|low| low.checked_add(bonus))
                    .is_some_and(|low| low >= start)
            })
        }
        // `literal >= symbol + offset` for every stored value would need the
        // symbol's ceiling, which a plain `u64` does not have inside i64.
        (TailBound::Literal(_), TailBound::Symbol { .. }) => false,
    }
}

/// `start >= 1`: a literal by its value, a symbol by its declared carrier
/// floor plus its constant shift (`i + 1` is positive for any unsigned `i`;
/// `step` needs a declared range flooring at 1).
fn tail_bound_is_positive(program: &dyn crate::CheckingInput, start: TailBound) -> bool {
    match start {
        TailBound::Literal(value) => value >= 1,
        TailBound::Symbol { symbol, offset } => bound_symbol_floor(program, symbol)
            .and_then(|floor| floor.checked_add(offset))
            .is_some_and(|floor| floor >= 1),
    }
}

/// The enforced lower bound of one immutable bound symbol's declared type: the
/// integer carrier's representable floor (unsigned types floor at zero),
/// tightened by an enforced declared range minimum. `None` for a non-integer
/// or unresolved binding -- never a guessed value.
fn bound_symbol_floor(program: &dyn crate::CheckingInput, symbol: SymbolHandle) -> Option<i64> {
    let type_reference = crate::value_custody::places::bound_symbol_declared_type(program, symbol)?;
    let carrier = match program
        .type_reference_table
        .primitive_type(type_reference)?
    {
        PrimitiveType::U8
        | PrimitiveType::U16
        | PrimitiveType::U32
        | PrimitiveType::U64
        | PrimitiveType::Addr => 0,
        PrimitiveType::I8 => i8::MIN as i64,
        PrimitiveType::I16 => i16::MIN as i64,
        PrimitiveType::I32 => i32::MIN as i64,
        PrimitiveType::I64 => i64::MIN,
        PrimitiveType::Selector | PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => {
            return None;
        }
    };
    let declared = crate::proof_contracts::arithmetic_domains::enforced_declared_range(
        program,
        type_reference,
    )
    .and_then(|interval| interval.low);
    Some(carrier.max(declared.unwrap_or(i64::MIN)))
}

fn names_parameter(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    parameter: &StateParameter,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Name(path)
            if path.symbol == parameter.symbol
                && path.head_symbol == parameter.symbol
                && program.expression_table.name_path_members(path.members).len() == 1
    )
}

fn parameter_is_slice(program: &dyn crate::CheckingInput, parameter: &StateParameter) -> bool {
    let mut reference = parameter.type_reference;
    let mut visited = Vec::new();
    while reference.is_valid() && !visited.contains(&reference) {
        visited.push(reference);
        reference = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Slice { .. } => return true,
            TypeReferenceNode::Reference { referee, .. } => *referee,
            TypeReferenceNode::Constrained { base_type, .. } => *base_type,
            _ => return false,
        };
    }
    false
}
