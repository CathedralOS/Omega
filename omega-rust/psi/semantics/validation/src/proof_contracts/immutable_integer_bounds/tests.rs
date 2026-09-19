//! Stable computed values are not compile-time integer constants.
use super::{
    ExpressionHandle, ExpressionNode, ImmutableIntegerBoundOffset, ImmutableIntegerBoundSum,
    StatementNode, SymbolHandle, TableLocalData, TypedTrees, immutable_integer_bound_sum,
    immutable_integer_bound_symbol_offset, immutable_integer_bound_value_symbol,
    normalize_immutable_integer_bound_expression, normalize_immutable_integer_bound_to_usize,
};
use typed_trees::expression::{BinaryOperator, Expression, NamePath, TableBinaryExpression};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};

fn name(
    program: &mut TypedTrees,
    spelling: &'static str,
    symbol: SymbolHandle,
) -> ExpressionHandle {
    program
        .expression_table
        .insert_tree(&Expression::Name(NamePath::resolved(
            vec![Identifier::generated_static(spelling)],
            symbol,
            symbol,
        )))
}

fn integer_type(
    program: &mut TypedTrees,
    domain: numerics::arithmetic::ArithmeticDomain,
) -> typed_trees::types::TypeReferenceHandle {
    let base = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated_static("u64"),
        });
    if domain == numerics::arithmetic::ArithmeticDomain::Exact {
        base
    } else {
        let constraints = program
            .type_reference_table
            .insert_constraints([TypeConstraintNode::ArithmeticDomain(domain)]);
        program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: base,
                constraints,
            })
    }
}

fn binary(
    program: &mut TypedTrees,
    left: ExpressionHandle,
    operator: BinaryOperator,
    right: ExpressionHandle,
) -> ExpressionHandle {
    program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator,
            right,
        }))
}

fn install_local(
    program: &mut TypedTrees,
    state: &mut State,
    symbol: SymbolHandle,
    spelling: &'static str,
    initial_value: ExpressionHandle,
    type_reference: typed_trees::types::TypeReferenceHandle,
    is_mutable: bool,
) {
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::LocalData(TableLocalData {
            symbol,
            name: Identifier::generated_static(spelling),
            type_reference,
            initial_value,
            is_mutable,
            ..Default::default()
        }),
    );
}

fn offset_fixture(
    domain: numerics::arithmetic::ArithmeticDomain,
    mutable: bool,
    parameter: bool,
) -> (TypedTrees, ExpressionHandle, SymbolHandle) {
    let mut program = TypedTrees::default();
    let ty = integer_type(&mut program, domain);
    let base_symbol = SymbolHandle::from_arena_index(20);
    let base = name(&mut program, "mid", base_symbol);
    let one = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(1),
    ));
    let expression = binary(&mut program, base, BinaryOperator::Add, one);
    let mut machine = Machine::default();
    let mut state = State::default();
    if parameter {
        program.push_state_parameter(
            &mut state,
            typed_trees::signature::StateParameter {
                symbol: base_symbol,
                name: Identifier::generated_static("mid"),
                type_reference: ty,
                is_mutable: mutable,
                ..Default::default()
            },
        );
    } else {
        install_local(
            &mut program,
            &mut state,
            base_symbol,
            "mid",
            one,
            ty,
            mutable,
        );
    }
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    (program, expression, base_symbol)
}

#[test]
fn computed_identity_preserves_copy_chains_without_becoming_a_static_index() {
    let mut program = TypedTrees::default();
    let one = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(1),
    ));
    let computed = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: one,
            operator: BinaryOperator::Add,
            right: one,
        }));
    let first_symbol = SymbolHandle::from_arena_index(1);
    let first = name(&mut program, "first", first_symbol);
    let copy_symbol = SymbolHandle::from_arena_index(2);
    let copy = name(&mut program, "copy", copy_symbol);
    let distinct_symbol = SymbolHandle::from_arena_index(3);
    let distinct = name(&mut program, "distinct", distinct_symbol);
    let literal_symbol = SymbolHandle::from_arena_index(4);
    let literal = name(&mut program, "literal", literal_symbol);
    let unresolved = name(&mut program, "ambiguous", SymbolHandle::invalid());
    let mut machine = Machine::default();
    let mut state = State::default();
    for (symbol, spelling, initial_value) in [
        (first_symbol, "first", computed),
        (copy_symbol, "copy", first),
        (distinct_symbol, "distinct", computed),
        (literal_symbol, "literal", one),
        (SymbolHandle::from_arena_index(5), "ambiguous", computed),
        (SymbolHandle::from_arena_index(6), "ambiguous", computed),
    ] {
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::LocalData(TableLocalData {
                symbol,
                name: Identifier::generated_static(spelling),
                initial_value,
                ..Default::default()
            }),
        );
    }
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    for (expression, expected) in [
        (first, first_symbol),
        (copy, first_symbol),
        (distinct, distinct_symbol),
    ] {
        assert_eq!(
            immutable_integer_bound_value_symbol(&program, expression),
            Some(expected)
        );
        assert!(normalize_immutable_integer_bound_expression(&program, expression).is_none());
        assert!(normalize_immutable_integer_bound_to_usize(&program, expression).is_none());
    }
    assert_eq!(
        normalize_immutable_integer_bound_to_usize(&program, literal),
        Some(1)
    );
    assert_eq!(
        normalize_immutable_integer_bound_expression(&program, literal),
        Some(one)
    );
    assert!(immutable_integer_bound_value_symbol(&program, literal).is_none());
    assert!(immutable_integer_bound_value_symbol(&program, computed).is_none());
    assert!(immutable_integer_bound_value_symbol(&program, unresolved).is_none());
    assert!(normalize_immutable_integer_bound_expression(&program, unresolved).is_none());
}

#[test]
fn immutable_copies_of_mutable_sources_are_values_not_static_indexes() {
    for parameter_source in [false, true] {
        let mut program = TypedTrees::default();
        let original_symbol = SymbolHandle::from_arena_index(1);
        let original = name(&mut program, "original", original_symbol);
        let cut_symbol = SymbolHandle::from_arena_index(2);
        let cut = name(&mut program, "cut", cut_symbol);
        let copy_symbol = SymbolHandle::from_arena_index(3);
        let copy = name(&mut program, "copy", copy_symbol);
        let later_symbol = SymbolHandle::from_arena_index(4);
        let later = name(&mut program, "later", later_symbol);
        let one = program.expression_table.insert(ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(1),
        ));
        let mut machine = Machine::default();
        let mut state = State::default();
        if parameter_source {
            program.push_state_parameter(
                &mut state,
                typed_trees::signature::StateParameter {
                    symbol: original_symbol,
                    is_mutable: true,
                    ..Default::default()
                },
            );
        } else {
            program.statement_table.push_statement(
                &mut state.statement_nodes,
                StatementNode::LocalData(TableLocalData {
                    symbol: original_symbol,
                    name: Identifier::generated_static("original"),
                    initial_value: one,
                    is_mutable: true,
                    ..Default::default()
                }),
            );
        }
        for (symbol, spelling, initial_value) in [
            (cut_symbol, "cut", original),
            (copy_symbol, "copy", cut),
            (later_symbol, "later", original),
        ] {
            program.statement_table.push_statement(
                &mut state.statement_nodes,
                StatementNode::LocalData(TableLocalData {
                    symbol,
                    name: Identifier::generated_static(spelling),
                    initial_value,
                    ..Default::default()
                }),
            );
        }
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
        for (expression, expected) in [
            (original, None),
            (cut, Some(cut_symbol)),
            (copy, Some(cut_symbol)),
            (later, Some(later_symbol)),
        ] {
            assert_eq!(
                immutable_integer_bound_value_symbol(&program, expression),
                expected
            );
            assert!(normalize_immutable_integer_bound_expression(&program, expression).is_none());
            assert!(normalize_immutable_integer_bound_to_usize(&program, expression).is_none());
        }
        // Ambiguous mutable origins must not become stable snapshot fallbacks.
        let mut duplicate_machine = Machine::default();
        let mut duplicate_state = State::default();
        if parameter_source {
            program.push_state_parameter(
                &mut duplicate_state,
                typed_trees::signature::StateParameter {
                    symbol: original_symbol,
                    is_mutable: false,
                    ..Default::default()
                },
            );
        } else {
            program.statement_table.push_statement(
                &mut duplicate_state.statement_nodes,
                StatementNode::LocalData(TableLocalData {
                    symbol: original_symbol,
                    initial_value: one,
                    ..Default::default()
                }),
            );
        }
        program.push_machine_state(&mut duplicate_machine, duplicate_state);
        program.push_machine(duplicate_machine);
        for expression in [original, cut, copy, later] {
            assert!(immutable_integer_bound_value_symbol(&program, expression).is_none());
            assert!(normalize_immutable_integer_bound_expression(&program, expression).is_none());
            assert!(normalize_immutable_integer_bound_to_usize(&program, expression).is_none());
        }
    }
}

#[test]
fn immutable_integer_bound_offsets_require_exact_symbolic_bases() {
    let mut program = TypedTrees::default();
    let ty = integer_type(&mut program, numerics::arithmetic::ArithmeticDomain::Exact);
    let mid_symbol = SymbolHandle::from_arena_index(30);
    let mid = name(&mut program, "mid", mid_symbol);
    let one = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(1),
    ));
    let expressions = [
        (binary(&mut program, mid, BinaryOperator::Add, one), 1),
        (binary(&mut program, one, BinaryOperator::Add, mid), 1),
        (binary(&mut program, mid, BinaryOperator::Subtract, one), -1),
    ];
    let reverse = binary(&mut program, one, BinaryOperator::Subtract, mid);
    let cut_symbol = SymbolHandle::from_arena_index(31);
    let cut = name(&mut program, "cut", cut_symbol);
    let mid_initial = binary(&mut program, one, BinaryOperator::Add, one);
    let mut machine = Machine::default();
    let mut state = State::default();
    install_local(
        &mut program,
        &mut state,
        mid_symbol,
        "mid",
        mid_initial,
        ty,
        false,
    );
    install_local(&mut program, &mut state, cut_symbol, "cut", mid, ty, false);
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    for (expression, offset) in expressions {
        assert_eq!(
            immutable_integer_bound_symbol_offset(&program, expression),
            Some(ImmutableIntegerBoundOffset {
                symbol: mid_symbol,
                offset,
            })
        );
    }
    assert!(immutable_integer_bound_symbol_offset(&program, reverse).is_none());
    let cut_offset = binary(&mut program, cut, BinaryOperator::Add, one);
    assert_eq!(
        immutable_integer_bound_symbol_offset(&program, cut_offset),
        Some(ImmutableIntegerBoundOffset {
            symbol: mid_symbol,
            offset: 1,
        })
    );
}

#[test]
fn immutable_integer_bound_offsets_reject_wrapping_and_mutable_bases() {
    for (domain, mutable, parameter) in [
        (
            numerics::arithmetic::ArithmeticDomain::Wrapping,
            false,
            false,
        ),
        (numerics::arithmetic::ArithmeticDomain::Exact, true, false),
        (numerics::arithmetic::ArithmeticDomain::Exact, true, true),
    ] {
        let (program, expression, _) = offset_fixture(domain, mutable, parameter);
        assert!(immutable_integer_bound_symbol_offset(&program, expression).is_none());
    }
}

#[test]
fn immutable_integer_bound_offsets_accept_exact_parameters() {
    let (program, expression, symbol) =
        offset_fixture(numerics::arithmetic::ArithmeticDomain::Exact, false, true);
    assert_eq!(
        immutable_integer_bound_symbol_offset(&program, expression),
        Some(ImmutableIntegerBoundOffset { symbol, offset: 1 })
    );
}

fn sum_fixture() -> (
    TypedTrees,
    ExpressionHandle,
    ExpressionHandle,
    ExpressionHandle,
    ExpressionHandle,
) {
    let mut program = TypedTrees::default();
    let ty = integer_type(&mut program, numerics::arithmetic::ArithmeticDomain::Exact);
    let i_symbol = SymbolHandle::from_arena_index(20);
    let j_symbol = SymbolHandle::from_arena_index(30);
    let k_symbol = SymbolHandle::from_arena_index(40);
    let i = name(&mut program, "i", i_symbol);
    let j = name(&mut program, "j", j_symbol);
    let k = name(&mut program, "k", k_symbol);
    let one = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(1),
    ));
    let mut machine = Machine::default();
    let mut state = State::default();
    for (symbol, spelling) in [(i_symbol, "i"), (j_symbol, "j"), (k_symbol, "k")] {
        program.push_state_parameter(
            &mut state,
            typed_trees::signature::StateParameter {
                symbol,
                name: Identifier::generated_static(spelling),
                type_reference: ty,
                ..Default::default()
            },
        );
    }
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    (program, i, j, k, one)
}

#[test]
fn immutable_integer_bound_sums_normalize_two_term_orderings() {
    let (mut program, i, j, _k, one) = sum_fixture();
    let i_plus_j = binary(&mut program, i, BinaryOperator::Add, j);
    let j_plus_i = binary(&mut program, j, BinaryOperator::Add, i);
    let expected = Some(ImmutableIntegerBoundSum {
        first: SymbolHandle::from_arena_index(20),
        second: SymbolHandle::from_arena_index(30),
        offset: 0,
    });
    // `i + j` and `j + i` share one canonical spelling.
    assert_eq!(immutable_integer_bound_sum(&program, i_plus_j), expected);
    assert_eq!(immutable_integer_bound_sum(&program, j_plus_i), expected);

    // Constants accumulate through association: `(i + 1) + (j + 2)` is
    // `i + j + 3`.
    let i_plus_one = binary(&mut program, i, BinaryOperator::Add, one);
    let two = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(2),
    ));
    let j_plus_two = binary(&mut program, j, BinaryOperator::Add, two);
    let shifted = binary(&mut program, i_plus_one, BinaryOperator::Add, j_plus_two);
    assert_eq!(
        immutable_integer_bound_sum(&program, shifted),
        Some(ImmutableIntegerBoundSum {
            first: SymbolHandle::from_arena_index(20),
            second: SymbolHandle::from_arena_index(30),
            offset: 3,
        })
    );

    // `i - 1 + j` subtracts only the constant: `i + j - 1`.
    let i_minus_one = binary(&mut program, i, BinaryOperator::Subtract, one);
    let shifted_down = binary(&mut program, i_minus_one, BinaryOperator::Add, j);
    assert_eq!(
        immutable_integer_bound_sum(&program, shifted_down),
        Some(ImmutableIntegerBoundSum {
            first: SymbolHandle::from_arena_index(20),
            second: SymbolHandle::from_arena_index(30),
            offset: -1,
        })
    );
}

#[test]
fn immutable_integer_bound_sums_reject_unspelled_terms() {
    let (mut program, i, j, k, _one) = sum_fixture();
    // `x + x` carries coefficient two: no spelling.
    let doubled = binary(&mut program, i, BinaryOperator::Add, i);
    assert!(immutable_integer_bound_sum(&program, doubled).is_none());
    // A third distinct symbol exceeds the two-term vocabulary.
    let i_plus_j = binary(&mut program, i, BinaryOperator::Add, j);
    let triple = binary(&mut program, i_plus_j, BinaryOperator::Add, k);
    assert!(immutable_integer_bound_sum(&program, triple).is_none());
    // `i - j` needs a negative coefficient on `j`.
    let difference = binary(&mut program, i, BinaryOperator::Subtract, j);
    assert!(immutable_integer_bound_sum(&program, difference).is_none());
    // `i * 2 + j` leaves an uninterpreted product.
    let doubled_i = binary(&mut program, i, BinaryOperator::Multiply, j);
    let product = binary(&mut program, doubled_i, BinaryOperator::Add, j);
    assert!(immutable_integer_bound_sum(&program, product).is_none());
    // A single term is the plain `symbol + offset` vocabulary, not a sum.
    assert!(immutable_integer_bound_sum(&program, i).is_none());
}

#[test]
fn immutable_integer_bound_sums_require_exact_immutable_terms() {
    for (domain, mutable) in [
        (numerics::arithmetic::ArithmeticDomain::Wrapping, false),
        (numerics::arithmetic::ArithmeticDomain::Exact, true),
    ] {
        let mut program = TypedTrees::default();
        let exact = integer_type(&mut program, numerics::arithmetic::ArithmeticDomain::Exact);
        let other = integer_type(&mut program, domain);
        let i_symbol = SymbolHandle::from_arena_index(20);
        let j_symbol = SymbolHandle::from_arena_index(30);
        let i = name(&mut program, "i", i_symbol);
        let j = name(&mut program, "j", j_symbol);
        let mut machine = Machine::default();
        let mut state = State::default();
        program.push_state_parameter(
            &mut state,
            typed_trees::signature::StateParameter {
                symbol: i_symbol,
                name: Identifier::generated_static("i"),
                type_reference: exact,
                ..Default::default()
            },
        );
        program.push_state_parameter(
            &mut state,
            typed_trees::signature::StateParameter {
                symbol: j_symbol,
                name: Identifier::generated_static("j"),
                type_reference: other,
                is_mutable: mutable,
                ..Default::default()
            },
        );
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
        let expression = binary(&mut program, i, BinaryOperator::Add, j);
        assert!(immutable_integer_bound_sum(&program, expression).is_none());
    }
}
