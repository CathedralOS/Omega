//! Initializer selection keeps computed initializers as authored roots and
//! relaxes no literal or declaration validity.

use crate::resolution::{
    lower_syntax_trees, lower_syntax_trees_for_const_argument_selection,
    lower_syntax_trees_for_const_initializer_selection,
};
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
};
use source::SourceId;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees::expression::ExpressionNode;
use syntax_trees::SyntaxTrees;

fn parse(sources: &[(SourceId, &str)]) -> SyntaxTrees {
    let mut syntax = SyntaxTrees::default();
    for (source, text) in sources {
        let tokens = Lexer::new(text).tokenize().expect("tokenize initializers");
        tokens_to_syntax_trees::parse_syntax_trees_into_with_id(&mut syntax, *source, &tokens)
            .expect("parse initializers");
    }
    syntax
}

#[test]
fn initializer_preparation_retains_forward_module_dependencies_without_values() {
    for reverse in [false, true] {
        let mut sources = [
            (
                SourceId(1),
                "module consumer; use settings::BASE; pub const COUNT: u64 = BASE + 1; const UNUSED: u64 = COUNT + 2;",
            ),
            (SourceId(2), "module settings; pub const BASE: u64 = 4;"),
        ];
        if reverse {
            sources.reverse();
        }
        let syntax = parse(&sources);
        assert!(lower_syntax_trees(&syntax).is_err());
        assert!(
            lower_syntax_trees_for_const_argument_selection(&syntax, None, Vec::new()).is_err()
        );
        let preparation =
            lower_syntax_trees_for_const_initializer_selection(&syntax, None, Vec::new())
                .expect("prepare selected initializer dependencies");
        let trees = preparation.trees();
        let base = trees
            .const_declarations
            .iter()
            .find(|declaration| trees.symbols.name(declaration.symbol) == "BASE")
            .expect("retain BASE declaration");
        assert!(base.canonical_value_encoding.is_some());
        for (name, dependency) in [("COUNT", "BASE"), ("UNUSED", "COUNT")] {
            let declaration = trees
                .const_declarations
                .iter()
                .find(|declaration| trees.symbols.name(declaration.symbol) == name)
                .expect("retain computed declaration");
            assert!(declaration.canonical_value_encoding.is_none());
            let ExpressionNode::Binary(binary) = trees
                .tables
                .bodies
                .expressions
                .expression(declaration.initializer)
            else {
                panic!("preparation must retain original binary initializer");
            };
            assert!(matches!(
                trees.tables.bodies.expressions.expression(binary.left),
                ExpressionNode::Name(_)
            ));
            let mut selections = trees
                .tables
                .bodies
                .expressions
                .authored_selection_occurrences(binary.left);
            assert!(selections.any(|occurrence| {
                let selection = trees
                    .authored_declaration_selections()
                    .get(occurrence)
                    .expect("retained selection");
                matches!(selection.target(), AuthoredDeclarationSelectionTarget::Resolved(selected)
                    if trees.symbols.name(selected.selected_symbol()) == dependency)
            }));
            assert!(
                trees
                    .tables
                    .bodies
                    .expressions
                    .authored_selection_occurrences(declaration.initializer)
                    .any(|occurrence| trees
                        .authored_declaration_selections()
                        .get(occurrence)
                        .is_some_and(|selection| selection.kind()
                            == AuthoredDeclarationSelectionKind::Operator))
            );
        }
    }
}

#[test]
fn initializer_preparation_does_not_relax_literal_or_declaration_validity() {
    for text in [
        "const BAD: u8 = 256; const PENDING: u64 = 1 + 2;",
        "const SAME: u64 = 1 + 2; const SAME: u64 = 3 + 4;",
        "const BAD: f32 = 1 + 2;",
    ] {
        let syntax = parse(&[(SourceId(1), text)]);
        assert!(
            lower_syntax_trees_for_const_initializer_selection(&syntax, None, Vec::new()).is_err(),
            "{text}"
        );
    }
}
