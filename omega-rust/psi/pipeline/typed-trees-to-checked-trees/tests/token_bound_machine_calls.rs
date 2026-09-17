//! A resolved use of a token-bearing machine is supplied by that machine's own
//! checked body: checking binds the spelled use to an ordinary call on the
//! declaration's entry state, retains the authored operator occurrence as a
//! selection of that declaration, and rejects the token positions whose body
//! supply is not implemented instead of leaving them to builtin arithmetic.

use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
};
use typed_trees::expression::ExpressionNode;

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )?;
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])?;
    typed_trees_to_checked_trees::lower_typed_trees(typed)
}

const WRAPPED_ADD: &str = "data Wrapped { value: u8; }
    machine + Wrapped::add(left: Wrapped, right: Wrapped) -> u64 {
        (left.value as u64) + (right.value as u64)
    }
    machine by_token(left: Wrapped, right: Wrapped) -> u64 { left + right }";

#[test]
fn binary_token_use_becomes_an_ordinary_call_on_the_declaration_entry() {
    let checked = check(WRAPPED_ADD).expect("the token use binds the declaration body");
    let add = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Wrapped::add")
        .expect("token-bearing declaration");
    let add_entry = checked.machine_states(add)[0].symbol;
    let synthesized = checked
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target_symbol == add_entry => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [call] = synthesized.as_slice() else {
        panic!(
            "exactly one call binds the token use; found {}",
            synthesized.len()
        );
    };
    assert_eq!(
        call.operational_acknowledgement.origin,
        language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized
    );
    assert_eq!(
        checked
            .expression_table
            .expression_handles(call.arguments)
            .len(),
        2,
        "both operands forward in authored order"
    );
    let remaining_additions = checked
        .expression_table
        .iter_expressions()
        .filter(|(_, expression)| {
            matches!(expression, ExpressionNode::Binary(binary)
                if binary.operator == typed_trees::expression::BinaryOperator::Add)
        })
        .count();
    assert_eq!(
        remaining_additions, 1,
        "only the builtin `u64 + u64` inside `Wrapped::add` stays a spelled operator"
    );
    let operator_selections = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Operator)
        .map(|selection| selection.target())
        .collect::<Vec<_>>();
    assert!(
        operator_selections.contains(&AuthoredDeclarationSelectionTarget::Resolved(
            language_semantics::declaration_selection::ResolvedAuthoredDeclarationSelection::new(
                add.symbol
            )
            .unwrap()
        )),
        "the authored `+` occurrence settles to the selected declaration: {operator_selections:?}"
    );
}

#[test]
fn index_token_use_rejects_instead_of_falling_back() {
    let diagnostics = check(
        "data Wrapped { value: u8; }
        machine [] Wrapped::at(target: Wrapped, index: u64) -> u8 { target.value }
        machine by_token(target: Wrapped) -> u8 { target[0u64] }",
    )
    .expect_err("an index token use has no implemented body supply");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "`Wrapped::at` was selected for its fixed operator token `[]` in a position whose body supply is not implemented"
        )),
        "{diagnostics:?}"
    );
}
