use crate::parser::data::parse_type_parameters;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::proof_fact::parse_proof_facts_until;
use crate::parser::state::{parse_optional_return_type, parse_optional_state_parameters};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::{CapabilityContract, CapabilityContractKind, OperatorDefinition};
use omega_syntax_trees::operator_spelling::OperatorSpelling;
use omega_tokens::PunctuationKind;

pub(super) fn parse_operator_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    is_boundary: bool,
) -> ParseResult<'tokens, 'source, OperatorDefinition> {
    let body_start_tokens = input.tokens.len();
    let (name, input) = parse_path_handle_span(input, |member| {
        syntax_trees.items.append_identifier_path_member(member)
    })?;
    let (type_parameters, input) = parse_type_parameters(syntax_trees, input)?;
    let (parameters, input) = parse_optional_state_parameters(syntax_trees, input)?;
    let (return_type, mut input) = parse_optional_return_type(syntax_trees, input)?;

    // `spelling` and `provider` clauses may appear before or after the
    // `requires`/`ensures` contracts (frozen Wave 0 decision #3 examples place
    // `spelling` before `requires`). Contracts stay contiguous in the arena
    // because the spelling/provider clauses are stored on the operator itself.
    let mut spelling = None;
    let mut provider = None;
    let mut contract_start = Handle::invalid();
    let mut contract_count = 0u32;
    loop {
        if is_operator_contract_start(&input) {
            let contract = parse_operator_contract(syntax_trees, &mut input)?;
            let handle = syntax_trees.items.append_capability_contract(contract);
            if contract_count == 0 {
                contract_start = handle;
            }
            contract_count = contract_count
                .checked_add(1)
                .expect("operator contract span count overflow");
            continue;
        }
        if input.at_contextual("spelling") {
            let after_keyword = input.take_contextual("spelling")?;
            let (parsed, rest) = parse_operator_spelling(after_keyword)?;
            if spelling.is_some() {
                return Err(rest.error_here("duplicate `spelling` clause on operator"));
            }
            spelling = Some(parsed);
            input = rest;
            continue;
        }
        if input.at_contextual("provider") {
            let after_keyword = input.take_contextual("provider")?;
            let (path, rest) = parse_path_handle_span(after_keyword, |member| {
                syntax_trees.items.append_identifier_path_member(member)
            })?;
            if provider.is_some() {
                return Err(rest.error_here("duplicate `provider` clause on operator"));
            }
            provider = Some(path);
            input = rest;
            continue;
        }
        break;
    }

    let contracts = if contract_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(contract_start, contract_count)
    };

    input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    let token_count = body_start_tokens.saturating_sub(input.tokens.len());

    Ok((
        OperatorDefinition {
            is_boundary,
            name,
            type_parameters,
            parameters,
            return_type,
            contracts,
            spelling,
            provider,
            token_count,
        },
        input,
    ))
}

/// Parses a `spelling` clause body, e.g. `+`, `[]`, `[..]`. Spelling symbols are
/// sequences of punctuation tokens; this assembles the lexemes and validates
/// against the legal set in [`OperatorSpelling`].
fn parse_operator_spelling<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, OperatorSpelling> {
    // `[]` and `[..]` span multiple punctuation tokens; everything else is a
    // single punctuation token. Greedily consume punctuation tokens that are
    // not the terminating semicolon and not a clause keyword start.
    let mut symbol = String::new();
    let mut rest = input;
    while rest.tokens.first().is_some_and(|token| {
        token.punctuation().is_some() && token.punctuation() != Some(PunctuationKind::Semicolon)
    }) {
        let (token, next) = rest.expect_token()?;
        symbol.push_str(token.lexeme.as_str());
        rest = next;
        if OperatorSpelling::from_symbol(&symbol).is_some() {
            break;
        }
    }

    match OperatorSpelling::from_symbol(&symbol) {
        Some(spelling) => Ok((spelling, rest)),
        None => Err(input.error_here(format!(
            "unknown operator spelling `{symbol}`; expected one of {}",
            OperatorSpelling::ALL
                .iter()
                .map(|spelling| spelling.symbol())
                .collect::<Vec<_>>()
                .join(" ")
        ))),
    }
}

fn parse_operator_contract<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: &mut Input<'tokens, 'source>,
) -> Result<CapabilityContract, crate::parse_error::ParseError> {
    if input.at_contextual("requires") {
        *input = input.take_contextual("requires")?;
        return parse_operator_fact_contract(syntax_trees, input, CapabilityContractKind::Requires);
    }

    if input.at_contextual("ensures") {
        *input = input.take_contextual("ensures")?;
        return parse_operator_fact_contract(syntax_trees, input, CapabilityContractKind::Ensures);
    }

    Err(input.expected_one_of_here(&["`requires`", "`ensures`"]))
}

fn parse_operator_fact_contract<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: &mut Input<'tokens, 'source>,
    kind: CapabilityContractKind,
) -> Result<CapabilityContract, crate::parse_error::ParseError> {
    let ((facts, token_count), rest) =
        parse_proof_facts_until(syntax_trees, *input, operator_contract_terminator)?;
    *input = rest;

    Ok(CapabilityContract {
        kind,
        facts,
        token_count,
    })
}

fn is_operator_contract_start(input: &Input<'_, '_>) -> bool {
    input.at_contextual("requires") || input.at_contextual("ensures")
}

fn operator_contract_terminator(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::Semicolon)
        || input.at_punctuation(PunctuationKind::RightBrace)
        || is_operator_contract_start(&input)
        || input.at_contextual("operator")
        || input.at_contextual("boundary")
        || input.at_contextual("spelling")
        || input.at_contextual("provider")
        || input.tokens.is_empty()
}
