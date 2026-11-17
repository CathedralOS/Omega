use crate::token_input::cursor::{Input, ParseResult};
use tokens::PunctuationKind;

pub(crate) fn parse_binding_relevance_brackets<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    site: &str,
) -> ParseResult<'tokens, 'source, language_core::BindingRelevance> {
    if !input.at_punctuation(PunctuationKind::LeftBracket) {
        return Ok((language_core::BindingRelevance::Relevant, input));
    }

    let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
    let mut relevance = language_core::BindingRelevance::Relevant;
    let mut declared_erased = false;
    while !input.at_punctuation(PunctuationKind::RightBracket) {
        let (name, next) = input.take_identifier()?;
        match name.as_str(&input) {
            "erased" => {
                if declared_erased {
                    return Err(next.error_here("duplicate binding property `erased`"));
                }
                declared_erased = true;
                relevance = language_core::BindingRelevance::Erased;
                input = next;
            }
            other => {
                return Err(next.error_here(format!(
                    "unknown {site} binding property `{other}`; declared binding properties are `erased`"
                )));
            }
        }

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }
        break;
    }

    let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
    Ok((relevance, input))
}
