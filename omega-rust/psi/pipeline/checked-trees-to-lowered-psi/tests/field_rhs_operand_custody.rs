//! Computed field assignments retain their authored scalar call operands.

use checked_trees::CheckedScalarComputationKind;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn computed_field_rhs_rejects_same_typed_call_operand_substitution() {
    let source = r#"
        data Main { first: u16; second: u16; }
        machine identity(value: u16) -> u16 { value }
        machine Main::main(&mut self, first: u16, second: u16) {
            self.first = identity(first);
            self.second = identity(second);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let mut checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let _ = terminal_production::produce_terminal_artifact(&checked, "Main::main")
        .expect("unmodified field RHS operands publish");

    let plans = &mut checked.facts.values.scalar_computations;
    let calls = plans
        .nodes
        .iter()
        .filter_map(|(handle, node)| {
            let CheckedScalarComputationKind::Call { arguments, .. } = &node.kind else {
                return None;
            };
            Some((handle, *arguments))
        })
        .collect::<Vec<_>>();
    let [(first_call, first_arguments), (_, second_arguments)] = calls.as_slice() else {
        panic!("fixture retains exactly two scalar calls")
    };
    let [first_operand] = plans.operands.span(*first_arguments).unwrap() else {
        panic!("first identity call has one operand")
    };
    let [second_operand] = plans.operands.span(*second_arguments).unwrap() else {
        panic!("second identity call has one operand")
    };
    assert_ne!(first_operand, second_operand);
    assert_eq!(
        plans.nodes.get(*first_operand).primitive_type,
        plans.nodes.get(*second_operand).primitive_type
    );

    // Substitute only the one-operand roster. Callee, source occurrence,
    // root, authored expression, result type, and destination remain unchanged.
    let CheckedScalarComputationKind::Call { arguments, .. } =
        &mut plans.nodes.get_mut(*first_call).kind
    else {
        unreachable!()
    };
    *arguments = *second_arguments;
    let result = terminal_production::produce_terminal_artifact(&checked, "Main::main");
    assert!(
        result.is_err(),
        "same-typed operand substitution must reject"
    );
}
