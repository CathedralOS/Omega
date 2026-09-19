pub use crate::diagnostics::parse_error;

use crate::ParseError;
use crate::declarations::parse_declaration::{ParsedDeclaration, parse_item};
use crate::input::token_cursor::Input;
use source::SourceId;
use syntax_trees::{SyntaxTrees, item::ItemHandle};
use tokens::Token;

/// Parses one source into the caller's arena and returns that source's root
/// item handles. Top-level `let`/`boundary let` declarations land on
/// `SyntaxTreeRoots::mathematical_definitions` instead — they are not items —
/// so they do not appear in the returned list. Existing roots retain their
/// identities. On error, the arena retains the parsed prefix.
pub fn parse(
    syntax_trees: &mut SyntaxTrees,
    source_id: SourceId,
    tokens: &[Token<'_>],
) -> Result<Vec<ItemHandle>, ParseError> {
    let mut input = Input::new(source_id, tokens);
    let mut root_items = Vec::new();
    while !input.tokens.is_empty() {
        let (declaration, rest) = parse_item(syntax_trees, input)?;
        match declaration {
            ParsedDeclaration::Item(item) => {
                root_items.push(syntax_trees.push_root_item(item));
            }
            ParsedDeclaration::Mathematical(definition) => {
                syntax_trees.push_root_mathematical_definition(definition);
            }
        }
        input = rest;
    }

    Ok(root_items)
}

pub use parse as parse_syntax_trees_into_with_id;

/// Creates a syntax arena for an anonymous source and parses its tokens.
pub fn parse_syntax_trees(tokens: &[Token<'_>]) -> Result<SyntaxTrees, ParseError> {
    parse_syntax_trees_with_id(SourceId::default(), tokens)
}

/// Creates a syntax arena for one identified source and parses its tokens.
pub fn parse_syntax_trees_with_id(
    source_id: SourceId,
    tokens: &[Token<'_>],
) -> Result<SyntaxTrees, ParseError> {
    let mut syntax_trees = SyntaxTrees::new(source_id);
    parse(&mut syntax_trees, source_id, tokens)?;
    Ok(syntax_trees)
}
