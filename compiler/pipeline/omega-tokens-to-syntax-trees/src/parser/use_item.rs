use crate::parser::input::{Input, ParseResult, parse_path};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::UseItem;
use omega_tokens::PunctuationKind;

pub(super) fn parse_use_item<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, UseItem> {
    let (path, input) = parse_path(input)?;
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    let path = syntax_trees
        .items
        .insert_identifier_path_members(path.iter().cloned());
    Ok((UseItem { path }, input))
}
