use crate::parser::input::{parse_path, ParseResult, Input};
use omega_syntax_trees::item::UseItem;
use omega_tokens::PunctuationKind;

pub(super) fn parse_use_item<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, UseItem> {
    let (path, input) = parse_path(input)?;
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((UseItem { path }, input))
}
