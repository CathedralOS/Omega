//! The typed decrease rule shared by runtime and proof slice recursion.

use language_semantics::declaration_selection::CollectionMeasure;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceNode};

/// A taken nonempty-slice guard makes its exact parameter's `start..` tail
/// strictly shorter whenever `start` is provably positive and the guard proves
/// `parameter.len >= start`.
///
/// Both bounds route through the shared immutable-integer-bound normalization
/// into an ordering key: an exact literal, or one immutable binding's value
/// identity (an immutable parameter leaf, or an immutable local whose
/// initializer stays symbolic) shifted by a compile-time constant under Exact
/// arithmetic. Two bounds naming the same symbol order by their offsets, so
/// `items[i + 1..]` admits under `items.len > i` (proving `len >= i + 1`) or
/// `items.len >= i + 1`. A symbolic bound also orders against a literal start
/// through its declared floor -- `len >= step` proves `len >= 2` when
/// `step: u64 [2..=8]` floors at 2. The `start >= 1` obligation uses the same
/// floor evidence: `i + 1` is positive for any unsigned `i`, while `step`
/// needs a declared range flooring at 1. Mutable, ambiguous, cross-symbol, and
/// literal-over-symbol orderings stay unknown rather than guessing a value
/// from spelling: `literal >= symbol` would need the symbol's ceiling, which a
/// plain `u64` does not have inside i64. A `0..` tail is the whole slice and
/// never decreases. The caller owns guard dominance and binding stability;
/// this predicate does not establish either from matching expression text.
pub fn slice_tail_strictly_decreases(
    program: &TypedTrees,
    guard: ExpressionHandle,
    argument: ExpressionHandle,
    parameter: &StateParameter,
) -> bool {
    if !parameter.symbol.is_valid() || !parameter_is_slice(program, parameter) {
        return false;
    }
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(argument) else {
        return false;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return false;
    };
    if !names_parameter(program, indexed.collection, parameter)
        || range.end.is_valid()
        || range.end_inclusive
    {
        return false;
    }
    let Some(start) = tail_bound(program, range.start) else {
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
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return false;
    };
    let ExpressionNode::Member(length) = program.expression_table.expression(binary.left) else {
        return false;
    };
    // `len > m` proves `len >= m + 1`, so it discharges `len >= start` when
    // `m + 1 >= start`; `len >= m` needs `m >= start` directly.
    let bonus = match binary.operator {
        BinaryOperator::Greater => 1,
        BinaryOperator::GreaterOrEqual => 0,
        _ => return false,
    };
    CollectionMeasure::from_authored_spelling(length.member.as_str())
        == Some(CollectionMeasure::Length)
        && names_parameter(program, length.receiver, parameter)
        && tail_bound(program, binary.right)
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
fn tail_bound(program: &TypedTrees, expression: ExpressionHandle) -> Option<TailBound> {
    let bound_lookup = crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup::new(
        program,
    );
    if let Some(value) =
        crate::normalize_immutable_integer_bound_to_usize(program, &bound_lookup, expression)
    {
        return Some(TailBound::Literal(value));
    }
    if let Some(bound) =
        crate::immutable_integer_bound_symbol_offset(program, &bound_lookup, expression)
    {
        return Some(TailBound::Symbol {
            symbol: bound.symbol,
            offset: bound.offset,
        });
    }
    if let Some(leaf) = crate::normalize_immutable_integer_bound_expression(
        program,
        &bound_lookup,
        expression,
    ) {
        return match program.expression_table.expression(leaf) {
            ExpressionNode::Name(path)
                if path.symbol.is_valid()
                    && path.head_symbol == path.symbol
                    && program
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        == 1 =>
            {
                Some(TailBound::Symbol {
                    symbol: path.symbol,
                    offset: 0,
                })
            }
            _ => None,
        };
    }
    crate::immutable_integer_bound_value_symbol(program, &bound_lookup, expression)
        .map(|symbol| TailBound::Symbol { symbol, offset: 0 })
}

/// `bound + bonus >= start` over ordering keys. `bonus` is 1 when the guard is
/// `len > bound` (which proves `len >= bound + 1`), 0 for `len >= bound`.
fn bound_ordering_at_least(
    program: &TypedTrees,
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
fn tail_bound_is_positive(program: &TypedTrees, start: TailBound) -> bool {
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
fn bound_symbol_floor(program: &TypedTrees, symbol: SymbolHandle) -> Option<i64> {
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
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => return None,
    };
    let declared = crate::proof_contracts::arithmetic_domains::enforced_declared_range(
        program,
        type_reference,
    )
    .and_then(|interval| interval.low);
    Some(carrier.max(declared.unwrap_or(i64::MIN)))
}

fn names_parameter(
    program: &TypedTrees,
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

fn parameter_is_slice(program: &TypedTrees, parameter: &StateParameter) -> bool {
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

#[cfg(test)]
mod tests {
    use super::{
        BinaryOperator, ExpressionHandle, ExpressionNode, StateParameter, TypeReferenceNode,
        TypedTrees, slice_tail_strictly_decreases,
    };
    use symbols::SymbolHandle;
    use typed_trees::expression::{
        Expression, NamePath, TableBinaryExpression, TableIndexedExpression, TableMemberExpression,
        TableRangeExpression,
    };
    use typed_trees::machine::Machine;
    use typed_trees::name::Identifier;
    use typed_trees::state::State;
    use typed_trees::statement::{StatementNode, TableLocalData};
    use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle};

    fn symbol(index: u32) -> SymbolHandle {
        SymbolHandle::from_arena_index(index)
    }

    fn integer(program: &mut TypedTrees, value: i64) -> ExpressionHandle {
        program.expression_table.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(value),
        ))
    }

    fn name(
        program: &mut TypedTrees,
        text: &'static str,
        symbol: SymbolHandle,
    ) -> ExpressionHandle {
        program
            .expression_table
            .insert_tree(&Expression::Name(NamePath::resolved(
                vec![Identifier::generated_static(text)],
                symbol,
                symbol,
            )))
    }

    fn length_of(program: &mut TypedTrees, receiver: ExpressionHandle) -> ExpressionHandle {
        program
            .expression_table
            .insert(ExpressionNode::Member(TableMemberExpression {
                receiver,
                member_symbol: SymbolHandle::invalid(),
                member: Identifier::generated_static("len"),
                case_variant: None,
            }))
    }

    fn length_guard(
        program: &mut TypedTrees,
        collection: ExpressionHandle,
        operator: BinaryOperator,
        bound: ExpressionHandle,
    ) -> ExpressionHandle {
        let length = length_of(program, collection);
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: length,
                operator,
                right: bound,
            }))
    }

    fn tail(
        program: &mut TypedTrees,
        collection: ExpressionHandle,
        start: ExpressionHandle,
    ) -> ExpressionHandle {
        let range = program
            .expression_table
            .insert(ExpressionNode::Range(TableRangeExpression {
                start,
                end: ExpressionHandle::invalid(),
                end_inclusive: false,
            }));
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection,
                index: range,
            }))
    }

    fn slice_parameter(program: &mut TypedTrees, symbol: SymbolHandle) -> StateParameter {
        let element = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated_static("u64"),
            });
        let slice = program
            .type_reference_table
            .insert(TypeReferenceNode::Slice {
                element_type: element,
            });
        StateParameter {
            symbol,
            name: Identifier::generated_static("items"),
            type_reference: slice,
            is_const: false,
            is_mutable: false,
            is_self: false,
            relevance: language_core::BindingRelevance::Relevant,
        }
    }

    fn install_locals(
        program: &mut TypedTrees,
        locals: impl IntoIterator<Item = (SymbolHandle, &'static str, ExpressionHandle, bool)>,
    ) {
        let element = integer_type(program, "u64");
        let mut machine = Machine::default();
        let mut state = State::default();
        for (symbol, name, initial_value, is_mutable) in locals {
            program.statement_table.push_statement(
                &mut state.statement_nodes,
                StatementNode::LocalData(TableLocalData {
                    symbol,
                    name: Identifier::generated_static(name),
                    type_reference: element,
                    initial_value,
                    is_mutable,
                    ..Default::default()
                }),
            );
        }
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
    }

    fn integer_type(program: &mut TypedTrees, name: &'static str) -> TypeReferenceHandle {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated_static(name),
            })
    }

    fn ranged_u64_type(
        program: &mut TypedTrees,
        minimum_value: i64,
        maximum_value: i64,
    ) -> TypeReferenceHandle {
        let minimum = integer(program, minimum_value);
        let maximum = integer(program, maximum_value);
        let base_type = integer_type(program, "u64");
        let mut constraints = arena::HandleSpan::default();
        program.type_reference_table.push_constraint(
            &mut constraints,
            TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive: true,
            },
        );
        program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type,
                constraints,
            })
    }

    fn install_parameters(
        program: &mut TypedTrees,
        parameters: impl IntoIterator<Item = (SymbolHandle, &'static str, TypeReferenceHandle, bool)>,
    ) {
        let mut machine = Machine::default();
        let mut state = State::default();
        for (symbol, name, type_reference, is_mutable) in parameters {
            program.push_state_parameter(
                &mut state,
                StateParameter {
                    symbol,
                    name: Identifier::generated_static(name),
                    type_reference,
                    is_const: false,
                    is_mutable,
                    is_self: false,
                    relevance: language_core::BindingRelevance::Relevant,
                },
            );
        }
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
    }

    fn shifted(program: &mut TypedTrees, base: ExpressionHandle, offset: i64) -> ExpressionHandle {
        let offset = integer(program, offset);
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: base,
                operator: BinaryOperator::Add,
                right: offset,
            }))
    }

    #[test]
    fn exact_literal_tail_still_decreases() {
        let mut program = TypedTrees::default();
        let parameter = slice_parameter(&mut program, symbol(1));
        let items = name(&mut program, "items", symbol(1));
        let start = integer(&mut program, 1);
        let argument = tail(&mut program, items, start);
        let items = name(&mut program, "items", symbol(1));
        let zero = integer(&mut program, 0);
        let guard = length_guard(&mut program, items, BinaryOperator::Greater, zero);

        assert!(slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));
    }

    #[test]
    fn wider_literal_tail_decreases_under_a_matching_guard() {
        let mut program = TypedTrees::default();
        let parameter = slice_parameter(&mut program, symbol(1));
        let items = name(&mut program, "items", symbol(1));
        let start = integer(&mut program, 2);
        let argument = tail(&mut program, items, start);
        let items = name(&mut program, "items", symbol(1));
        let two = integer(&mut program, 2);
        let guard = length_guard(&mut program, items, BinaryOperator::GreaterOrEqual, two);

        assert!(slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));
    }

    #[test]
    fn shared_immutable_bound_tail_decreases_under_the_same_guard_bound() {
        let mut program = TypedTrees::default();
        let parameter = slice_parameter(&mut program, symbol(1));
        let two = integer(&mut program, 2);
        install_locals(&mut program, [(symbol(2), "step", two, false)]);
        let items = name(&mut program, "items", symbol(1));
        let step = name(&mut program, "step", symbol(2));
        let argument = tail(&mut program, items, step);
        let items = name(&mut program, "items", symbol(1));
        let step = name(&mut program, "step", symbol(2));
        let guard = length_guard(&mut program, items, BinaryOperator::GreaterOrEqual, step);

        assert!(slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));
    }

    #[test]
    fn mutable_and_short_bounds_do_not_fake_a_decrease() {
        let mut program = TypedTrees::default();
        let parameter = slice_parameter(&mut program, symbol(1));
        let two = integer(&mut program, 2);
        install_locals(&mut program, [(symbol(2), "step", two, true)]);
        let items = name(&mut program, "items", symbol(1));
        let step = name(&mut program, "step", symbol(2));
        let argument = tail(&mut program, items, step);
        let items = name(&mut program, "items", symbol(1));
        let step = name(&mut program, "step", symbol(2));
        let guard = length_guard(&mut program, items, BinaryOperator::GreaterOrEqual, step);
        assert!(!slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));

        // A `2..` tail under `items.len >= 1` is not provably shorter: the
        // guard never establishes `items.len >= 2`, so the tail may not even
        // be a valid window, let alone a strict decrease.
        let items = name(&mut program, "items", symbol(1));
        let start = integer(&mut program, 2);
        let argument = tail(&mut program, items, start);
        let items = name(&mut program, "items", symbol(1));
        let one = integer(&mut program, 1);
        let guard = length_guard(&mut program, items, BinaryOperator::GreaterOrEqual, one);
        assert!(!slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));

        // `0..` is the whole slice: no decrease even under `len > 0`.
        let items = name(&mut program, "items", symbol(1));
        let start = integer(&mut program, 0);
        let argument = tail(&mut program, items, start);
        let items = name(&mut program, "items", symbol(1));
        let zero = integer(&mut program, 0);
        let guard = length_guard(&mut program, items, BinaryOperator::Greater, zero);
        assert!(!slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));
    }

    #[test]
    fn symbolic_shifted_start_decreases_under_a_strict_same_symbol_guard() {
        // `items[i + 1..]` under `items.len > i`: the strict guard proves
        // `len >= i + 1`, and the constant shift over `i`'s unsigned carrier
        // floor proves the start positive even though `i` stays unknown.
        let mut program = TypedTrees::default();
        let parameter = slice_parameter(&mut program, symbol(1));
        let u64_type = integer_type(&mut program, "u64");
        install_parameters(&mut program, [(symbol(2), "i", u64_type, false)]);
        let items = name(&mut program, "items", symbol(1));
        let i = name(&mut program, "i", symbol(2));
        let start = shifted(&mut program, i, 1);
        let argument = tail(&mut program, items, start);
        let items = name(&mut program, "items", symbol(1));
        let i = name(&mut program, "i", symbol(2));
        let guard = length_guard(&mut program, items, BinaryOperator::Greater, i);

        assert!(slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));
    }

    #[test]
    fn matching_symbolic_offsets_decrease_under_an_inclusive_guard() {
        // `items[i + 1..]` under `items.len >= i + 1`: the guard bound and the
        // tail start share `i`'s value identity, so their constant offsets
        // order directly.
        let mut program = TypedTrees::default();
        let parameter = slice_parameter(&mut program, symbol(1));
        let u64_type = integer_type(&mut program, "u64");
        install_parameters(&mut program, [(symbol(2), "i", u64_type, false)]);
        let items = name(&mut program, "items", symbol(1));
        let i = name(&mut program, "i", symbol(2));
        let start = shifted(&mut program, i, 1);
        let argument = tail(&mut program, items, start);
        let items = name(&mut program, "items", symbol(1));
        let i = name(&mut program, "i", symbol(2));
        let bound = shifted(&mut program, i, 1);
        let guard = length_guard(&mut program, items, BinaryOperator::GreaterOrEqual, bound);

        assert!(slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));
    }

    #[test]
    fn declared_floor_admits_an_unshifted_symbolic_start() {
        // `items[step..]` under `items.len >= step` with `step: u64 [1..=8]`:
        // the enforced declared range supplies the `step >= 1` evidence the
        // unsigned carrier floor alone cannot.
        let mut program = TypedTrees::default();
        let parameter = slice_parameter(&mut program, symbol(1));
        let step_type = ranged_u64_type(&mut program, 1, 8);
        install_parameters(&mut program, [(symbol(2), "step", step_type, false)]);
        let items = name(&mut program, "items", symbol(1));
        let step = name(&mut program, "step", symbol(2));
        let argument = tail(&mut program, items, step);
        let items = name(&mut program, "items", symbol(1));
        let step = name(&mut program, "step", symbol(2));
        let guard = length_guard(&mut program, items, BinaryOperator::GreaterOrEqual, step);

        assert!(slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));
    }

    #[test]
    fn symbolic_bounds_stay_unknown_without_positive_start_evidence() {
        let mut program = TypedTrees::default();
        let parameter = slice_parameter(&mut program, symbol(1));
        let u64_type = integer_type(&mut program, "u64");
        install_parameters(&mut program, [(symbol(2), "i", u64_type, false)]);

        // `items[i..]` under `items.len > i` with a plain `u64` `i`: `i` may
        // be zero, so the tail can be the whole slice. Value identity alone
        // never proves `start >= 1`.
        let items = name(&mut program, "items", symbol(1));
        let i = name(&mut program, "i", symbol(2));
        let argument = tail(&mut program, items, i);
        let items = name(&mut program, "items", symbol(1));
        let i = name(&mut program, "i", symbol(2));
        let guard = length_guard(&mut program, items, BinaryOperator::Greater, i);
        assert!(!slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));

        // `items[i + 1..]` under `items.len >= i`: `len` may equal `i`, so the
        // shifted start is not a proven in-range window.
        let items = name(&mut program, "items", symbol(1));
        let i = name(&mut program, "i", symbol(2));
        let start = shifted(&mut program, i, 1);
        let argument = tail(&mut program, items, start);
        let items = name(&mut program, "items", symbol(1));
        let i = name(&mut program, "i", symbol(2));
        let guard = length_guard(&mut program, items, BinaryOperator::GreaterOrEqual, i);
        assert!(!slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));

        // `items[j + 1..]` under `items.len > i`: different immutable symbols
        // do not order.
        let u64_type = integer_type(&mut program, "u64");
        install_parameters(&mut program, [(symbol(3), "j", u64_type, false)]);
        let items = name(&mut program, "items", symbol(1));
        let j = name(&mut program, "j", symbol(3));
        let start = shifted(&mut program, j, 1);
        let argument = tail(&mut program, items, start);
        let items = name(&mut program, "items", symbol(1));
        let i = name(&mut program, "i", symbol(2));
        let guard = length_guard(&mut program, items, BinaryOperator::Greater, i);
        assert!(!slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));
    }

    #[test]
    fn mutable_and_computed_bounds_do_not_fake_symbolic_identity() {
        let mut program = TypedTrees::default();
        let parameter = slice_parameter(&mut program, symbol(1));
        let u64_type = integer_type(&mut program, "u64");
        install_parameters(&mut program, [(symbol(2), "i", u64_type, true)]);

        // `items[i + 1..]` under `items.len > i` with `mut i`: a mutable bound
        // keeps no value identity, so the tail stays unknown.
        let items = name(&mut program, "items", symbol(1));
        let i = name(&mut program, "i", symbol(2));
        let start = shifted(&mut program, i, 1);
        let argument = tail(&mut program, items, start);
        let items = name(&mut program, "items", symbol(1));
        let i = name(&mut program, "i", symbol(2));
        let guard = length_guard(&mut program, items, BinaryOperator::Greater, i);
        assert!(!slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));

        // `items[j..]` under `items.len >= j` where `let j = <computed>` keeps
        // `j`'s value identity but its plain `u64` floor cannot prove
        // `j >= 1` -- the initializer's arithmetic is not read back into a
        // value.
        let zero = integer(&mut program, 0);
        let computed = shifted(&mut program, zero, 0);
        install_locals(&mut program, [(symbol(3), "j", computed, false)]);
        let items = name(&mut program, "items", symbol(1));
        let j = name(&mut program, "j", symbol(3));
        let argument = tail(&mut program, items, j);
        let items = name(&mut program, "items", symbol(1));
        let j = name(&mut program, "j", symbol(3));
        let guard = length_guard(&mut program, items, BinaryOperator::GreaterOrEqual, j);
        assert!(!slice_tail_strictly_decreases(
            &program, guard, argument, &parameter
        ));
    }
}
