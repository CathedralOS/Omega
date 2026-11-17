use crate::contracts::conformance::parse_optional_satisfies_type_arguments;
use crate::contracts::signatures::parse_signature_clauses;
use crate::parameters::callable::{parse_optional_parameters, parse_optional_return_type};
use crate::parameters::generics::GenericParameterSyntax;
use crate::parameters::generics::parse_generic_parameters;
use crate::token_input::cursor::{Input, ParseResult};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::MachineParameterContract;
use tokens::{KeywordKind, PunctuationKind};

pub(crate) fn parse_machine_parameter_contracts<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    type_parameters: HandleSpan<syntax_trees::item::TypeParameter>,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ()> {
    let mut input = input;
    loop {
        if !input.at_contextual("requires") {
            break;
        }
        let after_keyword = input.take_contextual("requires")?;
        if !after_keyword.at_keyword(KeywordKind::Machine) {
            break;
        }
        let after_machine = after_keyword.take_keyword(KeywordKind::Machine, "machine")?;
        let (name, after_name) = after_machine.take_identifier()?;

        if after_name.at_punctuation(PunctuationKind::ColonColon) {
            return Err(after_name.error_here(
                "one-off `requires machine T::member(...)` requirements are unsupported; declare a trait and bind an explicit conformance",
            ));
        }

        let Some(parameter_index) = syntax_trees
            .items
            .type_parameters(type_parameters)
            .iter()
            .position(|parameter| parameter.name.as_str(syntax_trees) == name.as_str(syntax_trees))
        else {
            return Err(after_name.error_here(format!(
                "`requires machine {}` has no matching generic parameter",
                name.as_str(syntax_trees)
            )));
        };

        match &syntax_trees.items.type_parameters(type_parameters)[parameter_index].kind {
            syntax_trees::item::TypeParameterKind::Machine {
                contract: None | Some(MachineParameterContract::RequirementIdentity),
            } => {}
            syntax_trees::item::TypeParameterKind::Type
                if syntax_trees.items.type_parameters(type_parameters)[parameter_index].bounds
                    == syntax_trees::item::DataProperties::default() => {}
            syntax_trees::item::TypeParameterKind::Machine { contract: Some(_) } => {
                return Err(after_name.error_here(format!(
                    "machine parameter `{}` already has a `requires machine` contract",
                    name.as_str(syntax_trees)
                )));
            }
            _ => {
                return Err(after_name.error_here(format!(
                    "`{}` is not a machine parameter; its declared kind or property bounds conflict with `requires machine {}`",
                    name.as_str(syntax_trees),
                    name.as_str(syntax_trees)
                )));
            }
        }

        if after_name.at_contextual("satisfies") {
            let after_satisfies = after_name.take_contextual("satisfies")?;
            let (requirement, trait_arguments, after_requirement) =
                parse_nominal_requirement_path(syntax_trees, after_satisfies)?;
            if requirement.len() < 2 {
                return Err(after_requirement.error_here(format!(
                    "nominal machine parameter `{}` must name an exact `Trait::requirement`; bare `satisfies Trait` is not a callable contract",
                    name.as_str(syntax_trees)
                )));
            }
            if after_requirement.at_contextual("as") {
                return Err(after_requirement.error_here(
                    "nominal machine parameter requirements do not accept `as Name`; `Trait::requirement` must resolve uniquely",
                ));
            }
            if after_requirement.at_contextual("via") {
                return Err(after_requirement.error_here(
                    "a `requires machine` nominal requirement cannot use `via`; native bindings belong to the owning Build",
                ));
            }
            let rest = take_machine_contract_terminator(after_requirement)?;
            let parameter =
                &mut syntax_trees.items.type_parameters_mut(type_parameters)[parameter_index];
            parameter.kind = syntax_trees::item::TypeParameterKind::Machine {
                contract: Some(MachineParameterContract::Nominal {
                    requirement,
                    trait_arguments,
                }),
            };
            input = rest;
            continue;
        }

        let (contract, rest) = parse_structural_machine_contract(syntax_trees, name, after_name)?;
        let rest = take_machine_contract_terminator(rest)?;
        let parameter =
            &mut syntax_trees.items.type_parameters_mut(type_parameters)[parameter_index];
        parameter.kind = syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(MachineParameterContract::Structural(contract)),
        };
        input = rest;
    }

    Ok(((), input))
}

pub(crate) fn parse_structural_machine_contract<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    name: syntax_trees::identifier::Identifier,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, syntax_trees::item::StateSignature> {
    let (nested_generic_parameters, after_type_parameters) =
        parse_generic_parameters(syntax_trees, input, GenericParameterSyntax::StaticBinders)?;
    let nested_type_parameters = nested_generic_parameters.type_parameters;
    let (parameters, after_parameters) =
        parse_optional_parameters(syntax_trees, after_type_parameters)?;
    let (return_type, after_return) = parse_optional_return_type(syntax_trees, after_parameters)?;
    let ((), after_nested_contracts) =
        parse_machine_parameter_contracts(syntax_trees, nested_type_parameters, after_return)?;
    let (clauses, rest) =
        parse_signature_clauses(syntax_trees, after_nested_contracts, nested_type_parameters)?;
    validate_machine_parameter_contracts(syntax_trees, nested_type_parameters, rest)?;
    if clauses.service_reach_is_installation_bound {
        return Err(rest.error_here(
                "`reaches <= Bound` is installation-selected and cannot appear on a structural machine parameter",
            ));
    }
    let contract = syntax_trees::item::StateSignature {
        name: name.clone(),
        lifetime_parameters: nested_generic_parameters.lifetime_parameters,
        type_parameters: nested_type_parameters,
        parameters,
        native_callback_parameters: Vec::new(),
        return_type,
        service_reach_keyword_source_spans: clauses.service_reach_keyword_source_spans,
        service_reaches: clauses.service_reaches,
        invokes: clauses.invokes,
        suspends_keyword_source_spans: clauses.suspends_keyword_source_spans,
        blocks_keyword_source_spans: clauses.blocks_keyword_source_spans,
        suspends: clauses.suspends,
        blocks: clauses.blocks,
        contracts: clauses.contracts,
        terminates_guarantee: clauses.terminates_guarantee,
        declares_no_crash_route: clauses.declares_no_crash_route,
        ..crate::parameters::callable::blank_state_signature()
    };
    Ok((contract, rest))
}

pub(crate) fn parse_typed_machine_contract<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    name: syntax_trees::identifier::Identifier,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, syntax_trees::item::StateSignature> {
    parse_callable_contract(
        syntax_trees,
        name,
        input,
        &[
            PunctuationKind::Greater,
            PunctuationKind::GreaterGreater,
            PunctuationKind::Comma,
        ],
    )
}

pub(crate) fn parse_runtime_machine_contract<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    name: syntax_trees::identifier::Identifier,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, syntax_trees::item::StateSignature> {
    parse_callable_contract(
        syntax_trees,
        name,
        input,
        &[
            PunctuationKind::Equal,
            PunctuationKind::Comma,
            PunctuationKind::RightParen,
            PunctuationKind::RightBracket,
            PunctuationKind::Greater,
            PunctuationKind::GreaterGreater,
        ],
    )
}

fn parse_callable_contract<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    name: syntax_trees::identifier::Identifier,
    input: Input<'tokens, 'source>,
    delimiters: &[PunctuationKind],
) -> ParseResult<'tokens, 'source, syntax_trees::item::StateSignature> {
    // Callable contracts use the ordinary machine grammar. Their facts
    // may contain comparisons and commas, so admit a boundary only after
    // the complete signature parses. Failed probes publish no syntax.
    let mut bounded = input;
    for delimiter in [PunctuationKind::LeftBrace, PunctuationKind::Semicolon] {
        if let Ok((prefix, _)) =
            bounded.split_at_top_level_punctuation(delimiter, "contract boundary")
        {
            bounded = prefix;
        }
    }
    let mut boundaries = vec![(bounded.tokens.len(), false)];
    for &delimiter in delimiters {
        let mut remaining = bounded;
        while let Ok((_, rest)) =
            remaining.split_at_top_level_punctuation(delimiter, "callable type delimiter")
        {
            let index = input.tokens.len()
                - rest.tokens.len()
                - (input.tokens.len() - bounded.tokens.len());
            let paired = delimiter == PunctuationKind::GreaterGreater;
            if paired {
                boundaries.push((index, false));
            }
            boundaries.push((index, paired));
            remaining = rest.restarted(&rest.tokens[1..]);
        }
    }
    boundaries.sort_unstable();
    for (index, paired) in boundaries.into_iter().rev() {
        let end = index + usize::from(paired);
        let mut prefix = input;
        prefix.tokens = &input.tokens[..end];
        let attempt = syntax_trees.append_checkpoint();
        if let Ok((contract, mut rest)) =
            parse_structural_machine_contract(syntax_trees, name.clone(), prefix)
            && rest.tokens.is_empty()
            && (!paired || rest.at_punctuation(PunctuationKind::Greater))
        {
            rest.resume_tokens(&input.tokens[end..]);
            return Ok((contract, rest));
        }
        syntax_trees.rewind_appended(attempt);
    }
    Err(input.error_here("expected a complete callable contract before its enclosing delimiter"))
}

pub(crate) fn validate_machine_parameter_contracts(
    syntax_trees: &SyntaxTrees,
    type_parameters: HandleSpan<syntax_trees::item::TypeParameter>,
    input: Input<'_, '_>,
) -> Result<(), crate::errors::parse_error::ParseError> {
    if let Some(missing) = syntax_trees
        .items
        .type_parameters(type_parameters)
        .iter()
        .find(|parameter| {
            matches!(
                parameter.kind,
                syntax_trees::item::TypeParameterKind::Machine { contract: None }
            )
        })
    {
        return Err(input.error_here(format!(
            "machine parameter `{}` requires an authored declaration-site contract: write `requires machine {}(...) -> Result;` or `requires machine {} satisfies Trait::requirement;`",
            missing.name.as_str(syntax_trees),
            missing.name.as_str(syntax_trees),
            missing.name.as_str(syntax_trees),
        )));
    }

    Ok(())
}

fn take_machine_contract_terminator<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> Result<Input<'tokens, 'source>, crate::errors::parse_error::ParseError> {
    input.take_punctuation(PunctuationKind::Semicolon, ";")
}

fn parse_nominal_requirement_path<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<
    (
        HandleSpan<syntax_trees::identifier::Identifier>,
        HandleSpan<syntax_trees::types::TypeReferenceHandle>,
        Input<'tokens, 'source>,
    ),
    crate::errors::parse_error::ParseError,
> {
    let (first, mut rest) = input.take_identifier()?;
    let mut members = vec![first];
    let mut trait_arguments = HandleSpan::empty();
    loop {
        if rest.at_punctuation(PunctuationKind::Less) {
            if !trait_arguments.is_empty() {
                return Err(rest
                    .error_here("a nominal machine parameter requirement applies its trait once"));
            }
            let (arguments, next) = parse_optional_satisfies_type_arguments(syntax_trees, rest)?;
            trait_arguments = arguments;
            rest = next;
            if !rest.at_punctuation(PunctuationKind::ColonColon) {
                return Err(rest.error_here(
                    "a trait application in a nominal machine parameter requirement must be followed by `::requirement`",
                ));
            }
        }
        if !rest.at_punctuation(PunctuationKind::ColonColon) {
            break;
        }
        rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (member, next) = rest.take_identifier()?;
        members.push(member);
        rest = next;
        if !trait_arguments.is_empty() {
            break;
        }
    }
    let mut path = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();
    for member in members {
        path.push(syntax_trees.items.append_identifier_path_member(member));
    }
    Ok((path.finish(), trait_arguments, rest))
}
