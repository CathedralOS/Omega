use crate::parser::data::parse_type_parameters;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::proof_fact::parse_proof_facts_until;
use crate::parser::state::{parse_optional_return_type, parse_optional_state_parameters};
use psi_arena::{Handle, HandleSpan};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::item::{
    CapabilityContract, CapabilityContractKind, CrashCause, OperatorDefinition,
};
use psi_syntax_trees::operator_spelling::OperatorSpelling;
use psi_tokens::PunctuationKind;

pub(super) fn parse_operator_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    is_boundary: bool,
) -> ParseResult<'tokens, 'source, OperatorDefinition> {
    let body_start_tokens = input.tokens.len();
    let (spelling, input) = if input
        .tokens
        .first()
        .is_some_and(|token| token.punctuation().is_some())
    {
        let (spelling, input) = parse_operator_spelling(input)?;
        (Some(spelling), input)
    } else {
        (None, input)
    };
    let (name, input) = parse_path_handle_span(input, |member| {
        syntax_trees.items.append_identifier_path_member(member)
    })?;
    let (generic_parameters, input) = parse_type_parameters(syntax_trees, input)?;
    let type_parameters = generic_parameters.type_parameters;
    let (parameters, input) = parse_optional_state_parameters(syntax_trees, input)?;
    let (return_type, mut input) = parse_optional_return_type(syntax_trees, input)?;

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
            is_public: false,
            is_boundary,
            name,
            lifetime_parameters: generic_parameters.lifetime_parameters,
            type_parameters,
            parameters,
            return_type,
            contracts,
            spelling,
            token_count,
        },
        input,
    ))
}

/// Parses the optional fixed-token declaration head, e.g. `+`, `[]`, `[..]`.
/// Fixed tokens are sequences of punctuation tokens; this assembles the
/// lexemes and validates against the closed [`OperatorSpelling`] set.
pub(super) fn parse_operator_spelling<'tokens, 'source>(
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
        let keyword_source_span = Some(input.current_source_span());
        *input = input.take_contextual("requires")?;
        return parse_operator_fact_contract(
            syntax_trees,
            input,
            CapabilityContractKind::Requires,
            keyword_source_span,
        );
    }

    if input.at_contextual("ensures") {
        let keyword_source_span = Some(input.current_source_span());
        *input = input.take_contextual("ensures")?;
        return parse_operator_fact_contract(
            syntax_trees,
            input,
            CapabilityContractKind::Ensures,
            keyword_source_span,
        );
    }

    if input.at_contextual("crashes") {
        let keyword_source_span = Some(input.current_source_span());
        *input = input.take_contextual("crashes")?;
        let (cause, after_cause) = input.take_identifier()?;
        let cause = match cause.as_str() {
            "Trap" => CrashCause::Trap,
            "Abort" => CrashCause::Abort,
            _ => {
                return Err(after_cause.error_here(format!(
                    "unknown crash cause `{}`; expected `Trap` or `Abort`",
                    cause.as_str()
                )));
            }
        };
        let after_header = after_cause;
        let header_token_count = 2usize;
        *input = after_header;
        let mut contract = parse_operator_fact_contract(
            syntax_trees,
            input,
            CapabilityContractKind::Crashes { cause },
            keyword_source_span,
        )?;
        contract.token_count = contract
            .token_count
            .checked_add(header_token_count)
            .expect("operator crash contract token count overflow");
        return Ok(contract);
    }

    Err(input.expected_one_of_here(&["`requires`", "`ensures`", "`crashes`"]))
}

fn parse_operator_fact_contract<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: &mut Input<'tokens, 'source>,
    kind: CapabilityContractKind,
    keyword_source_span: Option<psi_source::SourceSpan>,
) -> Result<CapabilityContract, crate::parse_error::ParseError> {
    let ((facts, token_count), rest) =
        parse_proof_facts_until(syntax_trees, *input, operator_contract_terminator)?;
    *input = rest;

    Ok(CapabilityContract {
        kind,
        keyword_source_span,
        binding: None,
        facts,
        token_count,
    })
}

fn is_operator_contract_start(input: &Input<'_, '_>) -> bool {
    input.at_contextual("requires")
        || input.at_contextual("ensures")
        || input.at_contextual("crashes")
}

fn operator_contract_terminator(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::Semicolon)
        || input.at_punctuation(PunctuationKind::RightBrace)
        || is_operator_contract_start(&input)
        || input.at_contextual("operator")
        || input.at_contextual("boundary")
        || input.tokens.is_empty()
}
