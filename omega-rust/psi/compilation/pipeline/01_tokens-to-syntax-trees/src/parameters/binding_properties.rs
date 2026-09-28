use crate::input::token_cursor::{Input, ParseResult};
use tokens::PunctuationKind;

/// Parse the closed property set that attaches to one authored binding
/// occurrence's name: `proof [erased]: Evidence` marks only `proof`, never
/// `Evidence` itself.
///
/// `[erased]` marks a binding occurrence, not a data-member kind
/// (wiki/spec/proofs/contracts.md#explicit-erased-bindings), so every
/// `name [properties]: Type` binding site shares this grammar. `site` names
/// the binding kind in diagnostics (`data-field`, `parameter`, `local`).
/// Binding properties are intentionally distinct from data/type properties:
/// `[copy]`/`[linear]`/`[carry(...)]` are type claims, not binding claims.
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
        match name.as_str() {
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
