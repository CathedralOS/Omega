use super::{BoundsCheckResult, RangeFacts, check_indexed_access};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableIndexedExpression};

mod lower_bounds;
mod selected;

fn fixture(
    collection_type: &str,
    access: &str,
) -> (
    typed_trees::TypedTrees,
    ExpressionHandle,
    TableIndexedExpression,
) {
    let source =
        format!("machine inspect(items: &{collection_type}, index: u64) {{ items{access}; }}");
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let (expression, indexed) = program
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| match node {
            ExpressionNode::Indexed(indexed) => Some((expression, *indexed)),
            _ => None,
        })
        .last()
        .expect("indexed expression");
    (program, expression, indexed)
}

fn result(collection_type: &str, access: &str, prove_index: bool) -> BoundsCheckResult {
    let (program, expression, indexed) = fixture(collection_type, access);
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let mut facts = RangeFacts::new(&[]);
    for parameter in program.state_parameters(state) {
        facts.define_local(
            parameter.symbol,
            parameter.name.to_string(),
            super::super::super::arrays::fixed_array_type_length(
                &program,
                parameter.type_reference,
            ),
            None,
        );
    }
    if prove_index {
        facts.prove_index("items".into(), "index".into());
    }
    // An unrelated diagnostic must not change this occurrence's proof result.
    let mut diagnostics = vec![diagnostics::Diagnostic::error("earlier error")];
    let result = check_indexed_access(
        &program,
        machine,
        state,
        &facts,
        expression,
        &indexed,
        &mut diagnostics,
    );
    assert_eq!(diagnostics.len() > 1, result == BoundsCheckResult::Rejected);
    result
}

#[test]
fn fixed_bounds_distinguish_scalar_elements_from_range_windows() {
    for (access, expected) in [
        ("[0]", BoundsCheckResult::ProvenScalar),
        ("[3]", BoundsCheckResult::ProvenScalar),
        ("[4]", BoundsCheckResult::Rejected),
        ("[-1]", BoundsCheckResult::Rejected),
        ("[0..4]", BoundsCheckResult::ProvenRange),
        ("[4..4]", BoundsCheckResult::ProvenRange),
        ("[0..5]", BoundsCheckResult::Rejected),
        ("[3..2]", BoundsCheckResult::Rejected),
    ] {
        assert_eq!(result("[u8; 4]", access, false), expected, "{access}");
    }
}

#[test]
fn dynamic_bounds_require_the_collection_relative_fact() {
    for collection in ["[u8; 4]", "[u8]"] {
        assert_eq!(
            result(collection, "[index]", false),
            BoundsCheckResult::Rejected
        );
        assert_eq!(
            result(collection, "[index]", true),
            BoundsCheckResult::ProvenScalar
        );
    }
}

#[test]
fn nested_collection_bounds_use_the_selected_element_type() {
    for (access, expected) in [
        ("[0][0]", BoundsCheckResult::ProvenScalar),
        ("[0][1]", BoundsCheckResult::ProvenScalar),
        ("[0][2]", BoundsCheckResult::Rejected),
        ("[0][0..2]", BoundsCheckResult::ProvenRange),
        ("[0][0..3]", BoundsCheckResult::Rejected),
    ] {
        assert_eq!(result("[[u8; 2]; 4]", access, false), expected, "{access}");
    }
}

#[test]
fn unrecognized_collection_is_not_a_bounds_admission() {
    assert_eq!(result("u64", "[0]", false), BoundsCheckResult::Unsupported);
}

#[test]
fn nested_index_traversal_checks_each_collection_extent() {
    for (access, accepted) in [("[3][1]", true), ("[4][1]", false), ("[3][2]", false)] {
        let (program, _, _) = fixture("[[u8; 2]; 4]", access);
        let frames = validation::CallFrameResolver::new(&program);
        let incoming = crate::checks::ranges::incoming_guards::IncomingGuardIndex::build(
            &program,
            frames.as_ref(),
        );
        let checked = crate::checks::ranges::check_indexed_accesses(
            &program,
            &checked_trees::CheckedOperatorFacts::default(),
            &checked_trees::BorrowFacts::default(),
            frames.as_ref(),
            &incoming,
        );
        assert_eq!(checked.is_ok(), accepted, "{access}: {checked:?}");
    }
}
