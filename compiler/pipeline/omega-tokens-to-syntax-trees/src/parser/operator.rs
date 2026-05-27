use crate::parser::input::{Input, ParseResult};
use omega_syntax_trees::item::OperatorDefinition;
use omega_tokens::PunctuationKind;

pub(super) fn parse_operator_definition<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, OperatorDefinition> {
    let body_start_tokens = input.tokens.len();
    let mut input = input;

    while !input.tokens.is_empty() {
        if input.at_contextual("intrinsic") {
            input = input.take_contextual("intrinsic")?;
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            let token_count = body_start_tokens.saturating_sub(input.tokens.len());
            return Ok((OperatorDefinition { token_count }, input));
        }

        let (_, rest) = input.expect_token()?;
        input = rest;
    }

    Err(input.error_here("operator declaration must end with `intrinsic;`"))
}
