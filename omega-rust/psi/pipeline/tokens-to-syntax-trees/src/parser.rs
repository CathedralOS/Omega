pub use crate::diagnostics::parse_error;

use crate::ParseError;
use crate::declarations;
use crate::input::Input;
use source::SourceId;
use syntax_trees::{SyntaxTrees, item::ItemHandle};
use tokens::Token;

/// Parses one source into the caller's arena and returns that source's root handles.
/// Existing roots retain their identities. On error, the arena retains the parsed prefix.
pub fn parse(
    syntax_trees: &mut SyntaxTrees,
    source_id: SourceId,
    tokens: &[Token<'_>],
) -> Result<Vec<ItemHandle>, ParseError> {
    let mut input = Input::new(source_id, tokens);
    let mut root_items = Vec::new();
    while !input.tokens.is_empty() {
        let (item, rest) = declarations::parse_item(syntax_trees, input)?;
        root_items.push(syntax_trees.push_root_item(item));
        input = rest;
    }

    Ok(root_items)
}

pub use crate::{parse_syntax_trees, parse_syntax_trees_with_id};
pub use parse as parse_syntax_trees_into_with_id;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
