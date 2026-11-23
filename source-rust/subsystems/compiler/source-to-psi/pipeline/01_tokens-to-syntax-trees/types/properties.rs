use crate::token_input::cursor::{Input, ParseResult};
use syntax_trees::item::DataProperties;
use tokens::PunctuationKind;

pub(crate) fn parse_property_brackets<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DataProperties> {
    let mut properties = DataProperties::default();
    let mut declared_multiplicity: Option<&'static str> = None;
    if !input.at_punctuation(PunctuationKind::LeftBracket) {
        return Ok((properties, input));
    }

    let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
    while !input.at_punctuation(PunctuationKind::RightBracket) {
        let (name, next) = input.take_identifier()?;
        match name.as_str(&input) {
            "copy" | "linear" => {
                let spelling = if name.as_str(&input) == "copy" {
                    "copy"
                } else {
                    "linear"
                };
                if let Some(previous) = declared_multiplicity {
                    let message = if previous == spelling {
                        format!("duplicate type property `{spelling}`")
                    } else {
                        format!(
                            "type properties `[copy]` and `[linear]` are mutually exclusive; \
                             `{previous}` was already declared"
                        )
                    };
                    return Err(next.error_here(message));
                }
                declared_multiplicity = Some(spelling);
                properties.multiplicity = if spelling == "copy" {
                    language_core::Multiplicity::Unrestricted
                } else {
                    language_core::Multiplicity::Linear
                };
                input = next;
            }
            "zero_init" => {
                return Err(next.error_here(
                    "type property `[zero_init]` is retired; whether zeroed storage establishes this type is derived from its default domain and zero-case payload",
                ));
            }
            // `wiki/spec/resources/suspension.md`: "Omega has no data property
            // or generic bound for suspension, CPU affinity, host-thread
            // affinity or address stability. `carry(...)` is invalid on
            // nominal data, transparent aliases and binders".
            "carry" => {
                return Err(input.error_here(
                    "`carry(...)` is not a type property: Omega has no data property or generic bound for suspension, CPU affinity, host-thread affinity or address stability",
                ));
            }
            "send" => {
                return Err(next.error_here(
                    "type property `[send]` is retired: Omega has no data property or generic bound for suspension or thread affinity",
                ));
            }
            "sized" => {
                return Err(next.error_here(
                    "type property `sized` is computed from the data shape and cannot be declared",
                ));
            }
            other => {
                return Err(next.error_here(format!(
                    "unknown type property `{other}`; declared properties are `copy` and `linear`"
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
    Ok((properties, input))
}
