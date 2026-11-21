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
            syntax_trees::item::TypeParameterKind::Machine { contract: Some(_) } => {
                return Err(after_name.error_here(format!(
                    "machine parameter `{}` already has a `requires machine` contract",
                    name.as_str(syntax_trees)
                )));
            }
            _ => {
                return Err(after_name.error_here(format!(
                    "`{}` is not a machine parameter; declare a callable with a typed static parameter",
                    name.as_str(syntax_trees)
                )));
            }
        }

        if after_name.at_contextual("satisfies") {
            return Err(after_name.error_here(format!(
                "declare this nominal callable in the parameter list as `const {}: Trait::requirement`",
                name.as_str(syntax_trees)
            )));
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
            "machine parameter `{}` requires an authored declaration-site contract: use a typed static parameter such as `const {}: Callback`",
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
