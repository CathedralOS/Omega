use crate::parse_error::ParseError;
use crate::parser::input::Input;
use psi_tokens::PunctuationKind;

pub(super) fn skip_delimited_block_after_open<'tokens, 'source>(
    mut input: Input<'tokens, 'source>,
    open: PunctuationKind,
    close: PunctuationKind,
) -> Result<(usize, Input<'tokens, 'source>), ParseError> {
    let mut depth = 1usize;
    let mut token_count = 0usize;

    while depth > 0 {
        let (token, rest) = input.expect_token()?;
        input = rest;

        match token.punctuation() {
            Some(punctuation) if punctuation == open => depth += 1,
            Some(punctuation) if punctuation == close => depth -= 1,
            _ => {}
        }

        if depth > 0 {
            token_count += 1;
        }
    }

    Ok((token_count, input))
}

pub(super) fn find_top_level_punctuation(
    input: Input<'_, '_>,
    delimiter: PunctuationKind,
) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;

    for (index, token) in input.tokens.iter().enumerate() {
        match token.punctuation() {
            Some(PunctuationKind::LeftParen) => paren_depth += 1,
            Some(PunctuationKind::RightParen) => paren_depth = paren_depth.saturating_sub(1),
            Some(PunctuationKind::LeftBracket) => bracket_depth += 1,
            Some(PunctuationKind::RightBracket) => bracket_depth = bracket_depth.saturating_sub(1),
            Some(PunctuationKind::LeftBrace) => {
                if delimiter == PunctuationKind::LeftBrace
                    && paren_depth == 0
                    && bracket_depth == 0
                    && brace_depth == 0
                {
                    return Some(index);
                }
                brace_depth += 1;
            }
            Some(PunctuationKind::RightBrace) => brace_depth = brace_depth.saturating_sub(1),
            Some(punctuation)
                if punctuation == delimiter
                    && paren_depth == 0
                    && bracket_depth == 0
                    && brace_depth == 0 =>
            {
                return Some(index);
            }
            _ => {}
        }
    }

    None
}
