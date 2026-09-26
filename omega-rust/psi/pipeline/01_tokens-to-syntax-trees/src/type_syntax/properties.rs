use crate::input::token_cursor::{Input, ParseResult};
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
        match name.as_str() {
            "copy" | "linear" => {
                let spelling = if name.as_str() == "copy" {
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
            "carry" => {
                if properties.carry.is_some() {
                    return Err(next.error_here("duplicate type property `carry`"));
                }
                let (policy, rest) = parse_carry_policy(next)?;
                properties.carry = Some(policy);
                input = rest;
            }
            "send" => {
                return Err(next.error_here(
                    "type property `[send]` is retired; declare the required four-axis `[carry(...)]` policy",
                ));
            }
            "sized" => {
                return Err(next.error_here(
                    "type property `sized` is computed from the data shape and cannot be declared",
                ));
            }
            other => {
                return Err(next.error_here(format!(
                    "unknown type property `{other}`; declared properties are `copy`, `linear`, `carry(...)`"
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

fn parse_carry_policy<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, language_core::CarryPolicy> {
    use language_core::{CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};

    let mut input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
    let mut suspension = None;
    let mut cpu = None;
    let mut host_thread = None;
    let mut address = None;

    while !input.at_punctuation(PunctuationKind::RightParen) {
        let (axis, next) = input.take_identifier()?;
        let next = next.take_punctuation(PunctuationKind::Colon, ":")?;
        let (value, rest) = next.take_identifier()?;
        match axis.as_str() {
            "suspension" => {
                if suspension.is_some() {
                    return Err(rest.error_here("duplicate carry axis `suspension`"));
                }
                suspension = Some(match value.as_str() {
                    "forbidden" => CarrySuspension::Forbidden,
                    "allowed" => CarrySuspension::Allowed,
                    other => {
                        return Err(rest.error_here(format!(
                            "unknown carry suspension `{other}`; expected `forbidden` or `allowed`"
                        )));
                    }
                });
            }
            "cpu" => {
                if cpu.is_some() {
                    return Err(rest.error_here("duplicate carry axis `cpu`"));
                }
                cpu = Some(match value.as_str() {
                    "same" => CarryCpu::Origin,
                    "any" => CarryCpu::Any,
                    other => {
                        return Err(rest.error_here(format!(
                            "unknown carry CPU affinity `{other}`; expected `same` or `any`"
                        )));
                    }
                });
            }
            "thread" => {
                if host_thread.is_some() {
                    return Err(rest.error_here("duplicate carry axis `thread`"));
                }
                host_thread = Some(match value.as_str() {
                    "same" => CarryHostThread::Origin,
                    "any" => CarryHostThread::Any,
                    other => {
                        return Err(rest.error_here(format!(
                            "unknown carry host-thread affinity `{other}`; expected `same` or `any`"
                        )));
                    }
                });
            }
            "address" => {
                if address.is_some() {
                    return Err(rest.error_here("duplicate carry axis `address`"));
                }
                address = Some(match value.as_str() {
                    "stable" => CarryAddress::Stable,
                    "movable" => CarryAddress::Movable,
                    other => {
                        return Err(rest.error_here(format!(
                            "unknown carry address policy `{other}`; expected `stable` or `movable`"
                        )));
                    }
                });
            }
            other => {
                return Err(rest.error_here(format!(
                    "unknown carry axis `{other}`; expected `suspension`, `cpu`, `thread`, or `address`"
                )));
            }
        }
        input = rest;
        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }
        break;
    }

    let missing = [
        ("suspension", suspension.is_none()),
        ("cpu", cpu.is_none()),
        ("thread", host_thread.is_none()),
        ("address", address.is_none()),
    ]
    .into_iter()
    .filter_map(|(axis, absent)| absent.then_some(axis))
    .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(input.error_here(format!(
            "carry policy must state all four axes; missing {}",
            missing.join(", ")
        )));
    }

    let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    Ok((
        CarryPolicy {
            suspension: suspension.expect("checked above"),
            cpu: cpu.expect("checked above"),
            host_thread: host_thread.expect("checked above"),
            address: address.expect("checked above"),
        },
        input,
    ))
}
