use crate::parser::data::parse_type_parameters;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::proof_fact::parse_proof_facts_until;
use crate::parser::state::{parse_optional_return_type, parse_optional_state_parameters};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::{CapabilityContract, CapabilityContractKind, OperatorDefinition};
use omega_tokens::PunctuationKind;

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

    while input.at_contextual("requires") || input.at_contextual("ensures") {
        let kind = if input.at_contextual("requires") {
            *input = input.take_contextual("requires")?;
            CapabilityContractKind::Requires
        } else {
            *input = input.take_contextual("ensures")?;
            CapabilityContractKind::Ensures
        };
        let ((facts, token_count), rest) =
            parse_proof_facts_until(syntax_trees, *input, |input| {
                input.at_punctuation(PunctuationKind::Semicolon)
                    || input.at_punctuation(PunctuationKind::RightBrace)
                    || input.at_contextual("requires")
                    || input.at_contextual("ensures")
                    || input.at_contextual("operator")
                    || input.tokens.is_empty()
            })?;
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
