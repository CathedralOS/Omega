use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::item::ExportItem;
use psi_tokens::PunctuationKind;

pub(super) fn parse_export_item<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExportItem> {
    let (path, mut input) = parse_path_handle_span(input, |member| {
        syntax_trees.items.append_identifier_path_member(member)
    })?;

    let alias = if input.at_contextual("as") {
        input = input.take_contextual("as")?;
        let (alias, rest) = input.take_identifier()?;
        input = rest;
        Some(alias)
    } else {
        None
    };

    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((ExportItem { path, alias }, input))
}
