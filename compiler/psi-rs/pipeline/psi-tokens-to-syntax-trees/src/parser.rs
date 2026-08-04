mod capability;
mod const_item;
mod context;
mod data;
mod diagnostics;
mod domain;
mod export_item;
mod expression;
mod file;
mod input;
mod invariant;
mod item;
mod library;
mod machine;
mod measure;
mod operator;
mod proof_fact;
mod proposition;
mod state;
mod statement;
mod target;
mod trait_definition;
mod transition;
mod type_reference;
mod use_item;

use crate::parse_error::ParseError;
use file::parse_file;
use input::Input;
use psi_source::SourceId;
use psi_syntax_trees::{SyntaxTrees, item::ItemHandle};
use psi_tokens::Token;

pub fn parse_syntax_trees(tokens: &[Token<'_>]) -> Result<SyntaxTrees, ParseError> {
    parse_syntax_trees_with_id(SourceId::default(), tokens)
}

pub fn parse_syntax_trees_with_id(
    source_id: SourceId,
    tokens: &[Token<'_>],
) -> Result<SyntaxTrees, ParseError> {
    let mut syntax_trees = SyntaxTrees::new(source_id);
    parse_syntax_trees_into_with_id(&mut syntax_trees, source_id, tokens)?;

    Ok(syntax_trees)
}

pub fn parse_syntax_trees_into_with_id(
    syntax_trees: &mut SyntaxTrees,
    source_id: SourceId,
    tokens: &[Token<'_>],
) -> Result<Vec<ItemHandle>, ParseError> {
    let input = Input::new(source_id, tokens);
    let mut root_items = Vec::new();
    let ((), rest) = parse_file(syntax_trees, input, &mut root_items)?;

    if let Some(token) = rest.tokens.first() {
        return Err(ParseError::at_source_span(
            "unexpected token after file parse",
            rest.source_span(token),
        ));
    }

    Ok(root_items)
}

#[cfg(test)]
mod tests;
