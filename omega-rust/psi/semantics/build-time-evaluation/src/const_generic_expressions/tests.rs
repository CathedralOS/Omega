use super::{exact_probe_destination, expression_custody};
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionLateBinding as Binding,
    AuthoredDeclarationSelectionTarget as Target,
};
use numerics::arithmetic::ArithmeticDomain;
use source_files_to_tokens::Lexer;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    types::{PrimitiveType, TypeConstraintNode, TypeReferenceNode},
};

fn typed_binary() -> (TypedTrees, ExpressionHandle) {
    let tokens = Lexer::new("machine run() -> u64 { transition { _ -> 1u64 + 2u64 } }")
        .tokenize()
        .expect("tokenize builtin probe");
    let syntax = parse_syntax_trees(&tokens).expect("parse builtin probe");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve builtin probe");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type builtin probe");
    let expression = typed
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| matches!(node, ExpressionNode::Binary(_)).then_some(handle))
        .expect("typed binary");
    (typed, expression)
}

#[test]
fn index_destination_rejects_range_constraints_even_under_exact_policy() {
    let (mut program, expression) = typed_binary();
    let machine = program.machines().iter().next().expect("probe machine");
    let destination = program.machine_states(machine)[0].return_type;
    assert_eq!(
        exact_probe_destination(&program, destination),
        Some(PrimitiveType::U64)
    );
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression).clone()
    else {
        panic!("binary bounds");
    };
    let constraints = program.type_reference_table.insert_constraints([
        TypeConstraintNode::Range {
            minimum: binary.left,
            maximum: binary.right,
        },
        TypeConstraintNode::ArithmeticDomain(ArithmeticDomain::Exact),
    ]);
    let constrained = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: destination,
            constraints,
        });
    assert_eq!(
        program.primitive_type_reference(constrained),
        Some(PrimitiveType::U64)
    );
    assert_eq!(
        program.type_reference_table.arithmetic_domain(constrained),
        ArithmeticDomain::Exact
    );
    assert_eq!(
        exact_probe_destination(&program, constrained),
        None,
        "a primitive projection and Exact policy do not discharge range obligations"
    );
}

#[test]
fn boolean_index_probe_retains_exact_literal_value_and_rejects_missing_nodes() {
    for value in [false, true] {
        let text = format!("machine run() -> bool {{ {value} }}");
        let tokens = Lexer::new(&text).tokenize().expect("Boolean probe tokens");
        let syntax = parse_syntax_trees(&tokens).expect("Boolean probe syntax");
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("Boolean probe resolution");
        let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("Boolean probe typing");
        let machine = program.machines().iter().next().expect("Boolean machine");
        let state = &program.machine_states(machine)[0];
        assert_eq!(
            exact_probe_destination(&program, state.return_type),
            Some(PrimitiveType::Bool)
        );
        let expression = program
            .expression_table
            .iter_expressions()
            .find_map(|(handle, node)| matches!(node, ExpressionNode::Boolean(_)).then_some(handle))
            .expect("Boolean literal");
        let (canonical, warnings) =
            super::value::evaluate(&program, machine, state, expression, PrimitiveType::Bool)
                .expect("selected Boolean literal");
        assert_eq!(
            canonical,
            language_semantics::const_value::CanonicalConstValue::boolean(value)
        );
        assert!(warnings.is_empty());
        assert!(
            super::value::evaluate(
                &program,
                machine,
                state,
                ExpressionHandle::invalid(),
                PrimitiveType::Bool
            )
            .is_err()
        );
    }
    let (program, expression) = typed_binary();
    let machine = program.machines().iter().next().expect("integer machine");
    let state = &program.machine_states(machine)[0];
    let (canonical, _) =
        super::value::evaluate(&program, machine, state, expression, PrimitiveType::Bool)
            .expect("landed integer retains its carrier for the caller's destination check");
    assert_eq!(canonical.type_name, "u64");
}

#[test]
fn genuine_builtin_binary_retains_exact_operator_custody() {
    let (program, expression) = typed_binary();
    let machine = program.machines().iter().next().expect("probe machine");
    let state = &program.machine_states(machine)[0];
    let expected = program
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| program.authored_declaration_selections().get(occurrence))
        .filter(|selection| selection.kind() == Kind::Operator)
        .map(|selection| selection.source_span())
        .collect::<Vec<_>>();
    assert_eq!(expected.len(), 1);
    let (constants, operators) = expression_custody(&program, machine, state, expression, false)
        .expect("checked builtin meaning");
    assert!(constants.is_empty());
    assert_eq!(operators, expected);
}

#[test]
fn folded_literal_cannot_promote_unresolved_operator_custody() {
    let (mut program, expression) = typed_binary();
    let machine = program
        .machines()
        .iter()
        .next()
        .expect("probe machine")
        .clone();
    let state = program.machine_states(&machine)[0].clone();
    assert!(
        program
            .expression_table
            .authored_selection_occurrences(expression)
            .any(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .is_some_and(|selection| {
                        selection.kind() == Kind::Operator
                            && selection.target() == Target::LateBound(Binding::CheckedOperator)
                    })
            })
    );
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        panic!("binary before forged fold");
    };
    let literal = program.expression_table.expression(binary.left).clone();
    assert!(matches!(literal, ExpressionNode::Integer(_)));
    for literal in [literal, ExpressionNode::Boolean(true)] {
        *program.expression_table.expression_mut(expression) = literal;
        let error = expression_custody(&program, &machine, &state, expression, false)
            .expect_err("folded literal has no checked operator meaning");
        assert!(error.contains("checked builtin meaning"));
    }
}

#[test]
fn data_index_discovery_excludes_shadowed_machine_scope() {
    let tokens = Lexer::new(
        r#"
        const SIZE: u64 = 2;
        data Buffer<const N: u64> { value: u64; }
        data Use { value: Buffer<SIZE + 0>; }
        machine run(SIZE: u64) -> u64 {
            let value: Buffer<SIZE + 1>;
            transition { _ -> 0u64 }
        }
    "#,
    )
    .tokenize()
    .expect("tokenize owner scopes");
    let syntax = parse_syntax_trees(&tokens).expect("parse owner scopes");
    let positions =
        syntax_trees_to_symbol_resolved_trees::closed_data_const_argument_expressions(&syntax);
    assert_eq!(
        positions.len(),
        1,
        "standalone probes cannot borrow a machine parameter's lexical scope"
    );
    let syntax_trees::types::TypeReferenceNode::ConstExpression(expression) =
        syntax.type_references.type_reference(positions[0].0)
    else {
        panic!("data index expression");
    };
    let syntax_trees::expression::ExpressionNode::Binary(binary) =
        syntax.expressions.expression(*expression)
    else {
        panic!("data index arithmetic");
    };
    let syntax_trees::expression::ExpressionNode::Integer(value) =
        syntax.expressions.expression(binary.right)
    else {
        panic!("data index literal");
    };
    assert_eq!(value.value_u64(), Some(0));
}
