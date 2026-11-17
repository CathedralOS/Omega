use crate::contracts::facts::parse_proof_facts_until;
use crate::parameters::callable::{parse_optional_parameters, parse_optional_return_type};
use crate::parameters::generics::GenericParameterSyntax;
use crate::parameters::generics::parse_generic_parameters;
use crate::token_input::cursor::{Input, ParseResult, parse_path_handle_span};
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{CapabilityContract, CapabilityContractKind, OperatorDefinition};
use syntax_trees::operator_spelling::OperatorSpelling;
use tokens::PunctuationKind;

pub(crate) fn parse_operator_definition<'tokens, 'source>(
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
    let (generic_parameters, input) =
        parse_generic_parameters(syntax_trees, input, GenericParameterSyntax::TypeAndConst)?;
    let type_parameters = generic_parameters.type_parameters;
    let (parameters, input) = parse_optional_parameters(syntax_trees, input)?;
    let (return_type, mut input) = parse_optional_return_type(syntax_trees, input)?;

    let mut contracts = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();
    loop {
        if is_operator_contract_start(&input) {
            let contract = parse_operator_contract(syntax_trees, &mut input)?;
            contracts.push(syntax_trees.items.append_capability_contract(contract));
            continue;
        }
        break;
    }

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
            contracts: contracts.finish(),
            spelling,
            token_count,
        },
        input,
    ))
}

pub(crate) fn parse_operator_spelling<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, OperatorSpelling> {
    let mut symbol = String::new();
    let mut rest = input;
    while rest.tokens.first().is_some_and(|token| {
        token.punctuation().is_some() && token.punctuation() != Some(PunctuationKind::Semicolon)
    }) {
        let (token, next) = rest.expect_token()?;
        symbol.push_str(input.text(token).as_str());
        rest = next;
        if OperatorSpelling::from_symbol(&symbol).is_some() {
            break;
        }
    }

    // A bindable arithmetic token followed by a policy suffix lexes as one
    // suffixed operator; name the rule instead of listing spellings.
    let policy_form = symbol.len() > 1
        && symbol.ends_with(['%', '|', '!'])
        && OperatorSpelling::from_symbol(&symbol[..symbol.len() - 1])
            .is_some_and(|base| matches!(base.symbol(), "+" | "-" | "*" | "/" | "%"));
    if policy_form {
        return Err(input.error_here(
                "a suffixed policy operator such as `+%`, `+|` or `+!` is a compiler-owned integer primitive; a declaration cannot bind one",
            ));
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
) -> Result<CapabilityContract, crate::errors::parse_error::ParseError> {
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
        if input.at_contextual("none") {
            return Err(input.error_here(
                    "`crashes none` is not an operator clause; an operator head without `crashes` declares no crash routes",
                ));
        }
        let (cause, after_cause) = input.take_identifier()?;
        let cause = crate::contracts::effects::parse_crash_cause(&cause, after_cause)?;
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
    keyword_source_span: Option<source::SourceSpan>,
) -> Result<CapabilityContract, crate::errors::parse_error::ParseError> {
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
