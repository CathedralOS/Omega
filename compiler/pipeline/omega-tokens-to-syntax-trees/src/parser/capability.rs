use crate::parser::input::{Input, ParseResult};
use crate::parser::state::parse_state_signature;
use crate::parser::type_reference::parse_type_reference;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::{
    CapabilityContract, CapabilityContractKind, CapabilityDefinition, CapabilityField,
    CapabilityMember, CapabilityState, TrustLevel,
};
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_capability_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, CapabilityDefinition> {
    let (name, mut input) = input.take_identifier()?;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut members = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_keyword(KeywordKind::State) || input.at_keyword(KeywordKind::Fn) {
            input = if input.at_keyword(KeywordKind::State) {
                input.take_keyword(KeywordKind::State, "state")?
            } else {
                input.take_keyword(KeywordKind::Fn, "fn")?
            };
            let (state, rest) = parse_capability_state(syntax_trees, input)?;
            members.push(CapabilityMember::State(state));
            input = rest;
        } else {
            let (field_name, rest) = input.take_identifier()?;
            let rest = rest.take_punctuation(PunctuationKind::Colon, ":")?;
            let (type_reference, rest) = parse_type_reference(rest)?;
            input = if rest.at_punctuation(PunctuationKind::Semicolon) {
                rest.take_punctuation(PunctuationKind::Semicolon, ";")?
            } else {
                rest
            };
            members.push(CapabilityMember::Field(CapabilityField {
                name: field_name,
                type_reference,
            }));
        }
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((CapabilityDefinition { name, members }, input))
}

fn parse_capability_state<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, CapabilityState> {
    let (signature, mut input) = parse_state_signature(syntax_trees, input)?;
    let mut contracts = Vec::new();

    if input.at_contextual("where") {
        input = input.take_contextual("where")?;
    }

    if input.at_punctuation(PunctuationKind::LeftBrace) {
        input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        while !input.at_punctuation(PunctuationKind::RightBrace) {
            let (contract, rest) = parse_capability_contract(input)?;
            contracts.push(contract);
            input = rest;
        }
        input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    } else {
        while !(input.at_keyword(KeywordKind::State)
            || input.at_keyword(KeywordKind::Fn)
            || input.at_punctuation(PunctuationKind::RightBrace)
            || input.tokens.is_empty())
        {
            let (contract, rest) = parse_capability_contract(input)?;
            contracts.push(contract);
            input = rest;
        }
    }

    Ok((
        CapabilityState {
            signature,
            contracts,
        },
        input,
    ))
}

fn parse_capability_contract<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, CapabilityContract> {
    if input.at_contextual("requires") {
        let input = input.take_contextual("requires")?;
        let (token_count, input) = skip_contract_tokens(input)?;
        return Ok((
            CapabilityContract {
                kind: CapabilityContractKind::Requires,
                token_count,
            },
            input,
        ));
    }

    if input.at_contextual("ensures") {
        let input = input.take_contextual("ensures")?;
        let (token_count, input) = skip_contract_tokens(input)?;
        return Ok((
            CapabilityContract {
                kind: CapabilityContractKind::Ensures,
                token_count,
            },
            input,
        ));
    }

    if input.at_keyword(KeywordKind::Trust) || input.at_contextual("trusted") {
        let input = if input.at_keyword(KeywordKind::Trust) {
            input.take_keyword(KeywordKind::Trust, "trust")?
        } else {
            input.take_contextual("trusted")?
        };
        let (trust_level, input) = parse_trust_level(input)?;
        return Ok((
            CapabilityContract {
                kind: CapabilityContractKind::Trusted(trust_level),
                token_count: 1,
            },
            input,
        ));
    }

    Err(input.error_here("expected capability contract"))
}

fn skip_contract_tokens<'tokens, 'source>(
    mut input: Input<'tokens, 'source>,
) -> Result<(usize, Input<'tokens, 'source>), crate::parse_error::ParseError> {
    let mut count = 0usize;
    while !(input.at_punctuation(PunctuationKind::Semicolon)
        || input.at_punctuation(PunctuationKind::RightBrace)
        || input.at_keyword(KeywordKind::State)
        || input.at_keyword(KeywordKind::Fn)
        || input.tokens.is_empty())
    {
        let (_, rest) = input.expect_token()?;
        input = rest;
        count += 1;
    }
    if input.at_punctuation(PunctuationKind::Semicolon) {
        input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    }
    Ok((count, input))
}

fn parse_trust_level<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TrustLevel> {
    if input.at_keyword(KeywordKind::Host) {
        let input = input.take_keyword(KeywordKind::Host, "host")?;
        Ok((TrustLevel::Host, input))
    } else {
        let (name, input) = input.take_identifier()?;
        Ok((TrustLevel::Named(name), input))
    }
}
