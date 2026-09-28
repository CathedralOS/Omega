//! A token-bearing trait requirement may be spelled on the ordinary `machine`
//! head.
//!
//! `expressions.md`'s executable-supply table gives the grammar as
//! `machine + Name(...)`, and a concrete crowned declaration already uses it.
//! The `operator` introducer the board is retiring was the only spelling a
//! TRAIT requirement accepted, so the two heads are checked here to agree all
//! the way through resolution and validation rather than only in the parser.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

fn typed_spellings(head: &str) -> Vec<Option<String>> {
    let source = format!(
        r#"
        trait Ranked {{
            {head} before(left: Self, right: Self) -> bool;
        }}

        data Card {{ rank: i32; }}

        Ascending: Card satisfies Ranked {{
            machine before(left: Card, right: Card) -> bool {{ left.rank < right.rank }}
        }}

        machine choose<Element, Order: Element satisfies Ranked>(
            left: Element,
            right: Element
        ) -> bool {{
            left < right
        }}
        "#
    );
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    validation::validate_program(&typed).expect("validate");
    let ranked = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Ranked")
        .expect("Ranked trait");
    typed
        .trait_machine_signatures(ranked)
        .iter()
        .map(|requirement| {
            requirement
                .spelling
                .map(|spelling| spelling.symbol().to_owned())
        })
        .collect()
}

#[test]
fn both_heads_record_the_same_requirement_token() {
    let ordinary = typed_spellings("machine <");
    assert_eq!(
        ordinary,
        vec![Some("<".to_owned())],
        "the ordinary machine head carries the token into typed trees"
    );
    assert_eq!(
        ordinary,
        typed_spellings("operator <"),
        "the retiring introducer records exactly the same thing"
    );
}
