use crate::token_input::cursor::Input;
use tokens::PunctuationKind;

pub(crate) fn find_top_level_punctuation(
    input: Input<'_, '_>,
    delimiter: PunctuationKind,
) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;

    for (index, token) in input.tokens.iter().enumerate() {
        let punctuation = token.punctuation();
        // At depth zero a closing token belongs to the caller. Check before
        // updating the depths so a close in this input stays part of it.
        if punctuation == Some(delimiter)
            && paren_depth == 0
            && bracket_depth == 0
            && brace_depth == 0
        {
            return Some(index);
        }
        match punctuation {
            Some(PunctuationKind::LeftParen) => paren_depth += 1,
            Some(PunctuationKind::RightParen) => paren_depth = paren_depth.saturating_sub(1),
            Some(PunctuationKind::LeftBracket) => bracket_depth += 1,
            Some(PunctuationKind::RightBracket) => bracket_depth = bracket_depth.saturating_sub(1),
            Some(PunctuationKind::LeftBrace) => brace_depth += 1,
            Some(PunctuationKind::RightBrace) => brace_depth = brace_depth.saturating_sub(1),
            _ => {}
        }
    }

    None
}
