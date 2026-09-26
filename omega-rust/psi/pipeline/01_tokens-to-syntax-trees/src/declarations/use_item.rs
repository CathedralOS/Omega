use crate::input::token_cursor::{Input, ParseResult, parse_path_handle_span};
use crate::syntax_trees::SyntaxTrees;
use crate::syntax_trees::item::UseItem;
use source_files_to_tokens::tokens::PunctuationKind;

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
