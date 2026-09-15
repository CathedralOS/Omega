use crate::input::token_cursor::{Input, ParseResult, parse_path_handle_span};
use crate::parameters::parse_generic_parameters::GenericParameterSyntax;
use crate::parameters::parse_generic_parameters::parse_generic_parameters;
use crate::parameters::parse_parameters::{parse_optional_parameters, parse_optional_return_type};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use tokens::{KeywordKind, PunctuationKind};

/// Parse the declaration-site contracts for static machine-symbol parameters.
/// The parameter list only names symbols (`<machine F>`); every such symbol
/// must receive exactly one authored `where machine F(...)` requirement before
/// the executable body's clauses begin. Nothing is inferred from uses.
pub(crate) fn parse_machine_parameter_contracts<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    type_parameters: HandleSpan<syntax_trees::item::TypeParameter>,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ()> {
    parse_machine_parameter_contracts_in(syntax_trees, type_parameters, input, true)
}

fn parse_machine_parameter_contracts_in<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    type_parameters: HandleSpan<syntax_trees::item::TypeParameter>,
    mut input: Input<'tokens, 'source>,
    reject_unknown_parameter: bool,
) -> ParseResult<'tokens, 'source, ()> {
    loop {
        if !input.at_contextual("where") {
            break;
        }
        let after_where = input.take_contextual("where")?;
        if !after_where.at_keyword(KeywordKind::Machine) {
            break;
        }
        let after_machine = after_where.take_keyword(KeywordKind::Machine, "machine")?;
        let (name, after_name) = after_machine.take_identifier()?;

        // A `::` after the name is not a machine-symbol parameter contract.
        // Leave it for the general clause parser, which rejects the retired
        // one-off member requirement rather than silently discarding it.
        if after_name.at_punctuation(PunctuationKind::ColonColon) {
            break;
        }

        let Some(parameter_index) = syntax_trees
            .items
            .type_parameters(type_parameters)
            .iter()
            .position(|parameter| parameter.name == name)
        else {
            if reject_unknown_parameter {
                return Err(after_name.error_here(format!(
                    "`where machine {}` has no matching `<machine {}>` parameter",
                    name.as_str(),
                    name.as_str()
                )));
            }
            // A nested signature's requirements are written in the same
            // clause stream as its owner's remaining requirements. An
            // unknown name at this level therefore belongs to the caller;
            // leave the complete `where machine` clause unconsumed.
            break;
        };

        match &syntax_trees.items.type_parameters(type_parameters)[parameter_index].kind {
            syntax_trees::item::TypeParameterKind::Machine { contract: None } => {}
            syntax_trees::item::TypeParameterKind::Machine { contract: Some(_) } => {
                return Err(after_name.error_here(format!(
                    "machine parameter `{}` already has a `where machine` contract",
                    name.as_str()
                )));
            }
            _ => {
                return Err(after_name.error_here(format!(
                    "`{}` is not a machine parameter; declare it as `<machine {}>` before writing a machine contract",
                    name.as_str(),
                    name.as_str()
                )));
            }
        }

        if after_name.at_contextual("satisfies") {
            let after_satisfies = after_name.take_contextual("satisfies")?;
            let (requirement, after_requirement) =
                parse_path_handle_span(after_satisfies, |member| {
                    syntax_trees.items.append_identifier_path_member(member)
                })?;
            if after_requirement.at_punctuation(PunctuationKind::Less) {
                return Err(after_requirement.error_here(
                    "generic trait arguments are not supported in nominal machine parameter requirements; name one exact `Trait::requirement`",
                ));
            }
            if requirement.len() < 2 {
                return Err(after_requirement.error_here(format!(
                    "nominal machine parameter `{}` must name an exact `Trait::requirement`; bare `satisfies Trait` is not a callable contract",
                    name.as_str()
                )));
            }
            if after_requirement.at_contextual("as") {
                return Err(after_requirement.error_here(
                    "nominal machine parameter requirements do not accept `as Name`; `Trait::requirement` must resolve uniquely",
                ));
            }
            if after_requirement.at_contextual("via") {
                return Err(after_requirement.error_here(
                    "a `where machine` nominal requirement cannot use `via`; bindings belong on the satisfying bodyless machine",
                ));
            }
            let after_semicolon =
                after_requirement.take_punctuation(PunctuationKind::Semicolon, ";")?;
            let rest = if continues_after_machine_parameter_contract(after_semicolon) {
                after_semicolon
            } else {
                // A bodyless declaration's final semicolon terminates both the
                // nominal binder and the declaration. Leave it for the outer
                // machine parser, just like operational clauses do.
                after_requirement
            };
            let parameter =
                &mut syntax_trees.items.type_parameters_mut(type_parameters)[parameter_index];
            parameter.kind = syntax_trees::item::TypeParameterKind::Machine {
                contract: Some(syntax_trees::item::MachineParameterContract::Nominal {
                    requirement,
                }),
            };
            input = rest;
            continue;
        }

        let (nested_generic_parameters, after_type_parameters) = parse_generic_parameters(
            syntax_trees,
            after_name,
            GenericParameterSyntax::StaticBinders,
        )?;
        let nested_type_parameters = nested_generic_parameters.type_parameters;
        let (parameters, after_parameters) =
            parse_optional_parameters(syntax_trees, after_type_parameters)?;
        let (return_type, after_return) =
            parse_optional_return_type(syntax_trees, after_parameters)?;
        let ((), after_nested_contracts) = parse_machine_parameter_contracts_in(
            syntax_trees,
            nested_type_parameters,
            after_return,
            false,
        )?;
        let (
            (
                service_reaches,
                service_reach_keyword_source_spans,
                service_reach_is_installation_bound,
                invokes,
                suspends_keyword_source_spans,
                blocks_keyword_source_spans,
                suspends,
                blocks,
                contracts,
                terminates_guarantee,
                where_facts,
            ),
            mut rest,
        ) = crate::contracts::signature::parse_signature_clauses(
            syntax_trees,
            after_nested_contracts,
            false,
        )?;
        if service_reach_is_installation_bound {
            return Err(rest.error_here(
                "`reaches <= Bound` is installation-selected and cannot appear on a structural machine parameter",
            ));
        }
        // Permit a separator after the requirement. The semicolon belongs to
        // this `where machine` signature, never to the generic machine body.
        if rest.at_punctuation(PunctuationKind::Semicolon) {
            rest = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;
        }

        let contract = syntax_trees::item::StateSignature {
            name: name.clone(),
            spelling: None,
            lifetime_parameters: nested_generic_parameters.lifetime_parameters,
            type_parameters: nested_type_parameters,
            is_default: false,
            parameters,
            native_callback_parameters: Vec::new(),
            return_type,
            service_reach_is_installation_bound: false,
            service_reach_keyword_source_spans,
            service_reaches,
            invokes,
            suspends_keyword_source_spans,
            blocks_keyword_source_spans,
            suspends,
            blocks,
            contracts,
            default_body: HandleSpan::empty(),
            terminates_guarantee,
            where_facts,
        };
        let parameter =
            &mut syntax_trees.items.type_parameters_mut(type_parameters)[parameter_index];
        parameter.kind = syntax_trees::item::TypeParameterKind::Machine {
            contract: Some(syntax_trees::item::MachineParameterContract::Structural(
                contract,
            )),
        };
        input = rest;
    }

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
            "machine parameter `{}` requires an authored declaration-site contract: write `where machine {}(...) -> Result` or `where machine {} satisfies Trait::requirement;`",
            missing.name.as_str(),
            missing.name.as_str(),
            missing.name.as_str(),
        )));
    }

    Ok(((), input))
}

fn continues_after_machine_parameter_contract(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::LeftBrace)
        || input.at_punctuation(PunctuationKind::Arrow)
        || input.at_contextual("terminates")
        || input.at_contextual("decreases")
        || input.at_contextual("reaches")
        || input.at_contextual("effects")
        || input.at_contextual("invokes")
        || input.at_contextual("suspends")
        || input.at_contextual("blocks")
        || input.at_contextual("crashes")
        || input.at_contextual("boundary")
        || input.at_contextual("requires")
        || input.at_contextual("ensures")
        || input.at_contextual("where")
        || input.at_contextual("satisfies")
}
