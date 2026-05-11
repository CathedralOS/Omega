use crate::parser::input::{Input, ParseResult};
use crate::parser::state::parse_state_signature;
use omega_syntax_trees::item::Platform;
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_platform<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Platform> {
    let (name, mut input) = input.take_identifier()?;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut states = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_keyword(KeywordKind::Pub) {
            input = input.take_keyword(KeywordKind::Pub, "pub")?;
            if input.at_keyword(KeywordKind::Entry) {
                input = input.take_keyword(KeywordKind::Entry, "entry")?;
            }
        } else if input.at_keyword(KeywordKind::Entry) {
            input = input.take_keyword(KeywordKind::Entry, "entry")?;
        } else {
            input = input.take_keyword(KeywordKind::Fn, "fn")?;
        }

        let (signature, rest) = parse_state_signature(input)?;
        states.push(signature);
        input = if rest.at_punctuation(PunctuationKind::Semicolon) {
            rest.take_punctuation(PunctuationKind::Semicolon, ";")?
        } else {
            rest
        };
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((Platform { name, states }, input))
}
