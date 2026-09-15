use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use syntax_trees::SyntaxTrees;
use syntax_trees::item::UseItem;
use tokens::PunctuationKind;

pub(super) fn parse_use_item<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, UseItem> {
    let (path, input) = parse_path_handle_span(input, |member| {
        syntax_trees.items.append_identifier_path_member(member)
    })?;
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((UseItem { path }, input))
}
