use crate::parser::data::parse_type_parameters;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::proof_fact::parse_proof_facts_until;
use crate::parser::state::{parse_optional_return_type, parse_optional_state_parameters};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::{
    CapabilityContract, CapabilityContractKind, OperatorDefinition, TrustLevel,
};
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_operator_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, OperatorDefinition> {
    let body_start_tokens = input.tokens.len();
    let (name, input) = parse_path_handle_span(input, |member| {
        syntax_trees.items.append_identifier_path_member(member)
    })?;
    let (type_parameters, input) = parse_type_parameters(syntax_trees, input)?;
    let (parameters, input) = parse_optional_state_parameters(syntax_trees, input)?;
    let (return_type, mut input) = parse_optional_return_type(syntax_trees, input)?;
    let contracts = parse_operator_contracts(syntax_trees, &mut input)?;

    input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    let token_count = body_start_tokens.saturating_sub(input.tokens.len());

    Ok((
        OperatorDefinition {
            name,
            type_parameters,
            parameters,
            return_type,
            contracts,
            token_count,
        },
        input,
    ))
}

fn parse_operator_contracts<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: &mut Input<'tokens, 'source>,
) -> Result<HandleSpan<CapabilityContract>, crate::parse_error::ParseError> {
    let mut contract_start = Handle::invalid();
    let mut contract_count = 0u32;

    while input.at_contextual("requires")
        || input.at_contextual("ensures")
        || input.at_keyword(KeywordKind::Trust)
        || input.at_contextual("trusted")
    {
        let kind = if input.at_contextual("requires") {
            *input = input.take_contextual("requires")?;
            CapabilityContractKind::Requires
        } else if input.at_contextual("ensures") {
            *input = input.take_contextual("ensures")?;
            CapabilityContractKind::Ensures
        } else {
            *input = if input.at_keyword(KeywordKind::Trust) {
                input.take_keyword(KeywordKind::Trust, "trust")?
            } else {
                input.take_contextual("trusted")?
            };
            let (trust, rest) = parse_trust_level(*input)?;
            *input = rest;
            CapabilityContractKind::Trusted(trust)
        };

        let (facts, token_count, rest) = if matches!(kind, CapabilityContractKind::Trusted(_)) {
            (HandleSpan::empty(), 1, *input)
        } else {
            let ((facts, token_count), rest) =
                parse_proof_facts_until(syntax_trees, *input, |input| {
                    input.at_punctuation(PunctuationKind::Semicolon)
                        || input.at_punctuation(PunctuationKind::RightBrace)
                        || input.at_contextual("requires")
                        || input.at_contextual("ensures")
                        || input.at_keyword(KeywordKind::Trust)
                        || input.at_contextual("trusted")
                        || input.at_contextual("operator")
                        || input.tokens.is_empty()
                })?;
            (facts, token_count, rest)
        };
        *input = rest;

        let handle = syntax_trees
            .items
            .append_capability_contract(CapabilityContract {
                kind,
                facts,
                token_count,
            });
        if contract_count == 0 {
            contract_start = handle;
        }
        contract_count = contract_count
            .checked_add(1)
            .expect("operator contract span count overflow");
    }

    Ok(if contract_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(contract_start, contract_count)
    })
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
