//! Shared const/value argument boundary: shifts win when complete; otherwise
//! a lexed `>>` may close nested applications without consuming either closer.

use crate::diagnostics::parse_error::ParseError;
use crate::grammar::expressions::parse_expression::parse_const_integer_expression_handle;
use crate::tokens::token_cursor::Input;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use tokens::PunctuationKind;

pub(crate) fn parse_static_argument_expression<'tokens, 'source>(
    scratch: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<Option<(ExpressionHandle, Input<'tokens, 'source>)>, ParseError> {
    let whole_attempt = scratch.append_checkpoint();
    let parsed = parse_const_integer_expression_handle(scratch, input);
    let (expression, rest) = match parsed {
        Ok(parsed)
            if parsed.1.at_punctuation(PunctuationKind::Comma)
                || parsed.1.at_punctuation(PunctuationKind::Greater) =>
        {
            parsed
        }
        _ => {
            scratch.rewind_appended(whole_attempt);
            // A lexed `>>` may close two applications instead of shifting.
            // Prefer a complete expression; otherwise retry the delimited
            // prefix transactionally, leaving both closers for their owners.
            let mut boundaries = Vec::new();
            let mut remaining = input;
            while let Ok((_, boundary)) = remaining.split_at_top_level_punctuation(
                PunctuationKind::GreaterGreater,
                "static argument delimiter",
            ) {
                boundaries.push(boundary);
                remaining = boundary.take_punctuation(PunctuationKind::GreaterGreater, ">>")?;
            }
            let mut selected = None;
            for rest in boundaries.into_iter().rev() {
                let mut prefix = input;
                prefix.tokens = &input.tokens[..input.tokens.len() - rest.tokens.len()];
                let attempt = scratch.append_checkpoint();
                if let Ok((expression, end)) =
                    parse_const_integer_expression_handle(scratch, prefix)
                    && end.tokens.is_empty()
                    && !end.at_punctuation(PunctuationKind::Greater)
                {
                    selected = Some((expression, rest));
                    break;
                }
                scratch.rewind_appended(attempt);
            }
            let Some(selected) = selected else {
                return Ok(None);
            };
            selected
        }
    };
    Ok(Some((expression, rest)))
}
