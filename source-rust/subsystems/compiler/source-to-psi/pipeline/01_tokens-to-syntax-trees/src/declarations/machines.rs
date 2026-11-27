use crate::declarations::machine_bodies::parse_body;
use crate::declarations::operators::parse_operator_spelling;
use crate::parameters::callable::{parse_optional_parameters, parse_optional_return_type};
use crate::parameters::generics::GenericParameterSyntax;
use crate::parameters::generics::parse_generic_parameters;
use crate::token_input::cursor::{Input, ParseResult, parse_path_handle_span};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{Machine, State};
use tokens::PunctuationKind;

use crate::contracts::clauses::parse_machine_clauses;
use crate::contracts::conformance::parse_satisfies_traits;
use crate::parameters::requirements::parse_machine_parameter_contracts;
use crate::parameters::requirements::validate_machine_parameter_contracts;

pub(crate) fn parse_machine<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Machine> {
    let declaration_start = input;
    let (spelling, input) = if !input.at_path_head()
        && input.tokens.first().is_some_and(|token| {
            token.punctuation().is_some() && token.punctuation() != Some(PunctuationKind::Semicolon)
        })
    {
        let (spelling, input) = parse_operator_spelling(input)?;
        (Some(spelling), input)
    } else {
        (None, input)
    };
    let (path, input) = parse_path_handle_span(input, |member| {
        syntax_trees
            .expressions
            .append_identifier_path_member(member)
    })?;
    let (generic_parameters, input) = parse_generic_parameters(
        syntax_trees,
        input,
        GenericParameterSyntax::MachineDeclaration,
    )?;
    let type_parameters = generic_parameters.type_parameters;
    let lifetime_parameters = generic_parameters.lifetime_parameters;
    let (machine_parameters, input) = parse_optional_parameters(syntax_trees, input)?;
    let (machine_return_type, input) = parse_optional_return_type(syntax_trees, input)?;
    let ((), mut input) = parse_machine_parameter_contracts(syntax_trees, type_parameters, input)?;
    let (satisfies, next) = parse_satisfies_traits(syntax_trees, input)?;
    if spelling.is_some() && !satisfies.is_empty() {
        return Err(next.error_here(
            "a machine with a `satisfies` clause is a realization and cannot declare an \
             operator token; the satisfied requirement owns the token binding",
        ));
    }
    if next.at_contextual("via") {
        return Err(next.error_here(
            "`via <Binding>` cannot supply a machine by itself; name the requirement first as \
             `satisfies Trait::machine via Binding::Case(...)`",
        ));
    }
    let (clauses, next) = parse_machine_clauses(syntax_trees, next, type_parameters)?;
    validate_machine_parameter_contracts(syntax_trees, type_parameters, next)?;
    let conformance_bounds = generic_parameters.conformance_bounds;
    input = next;
    let machine_return_type = if machine_return_type.is_valid() {
        machine_return_type
    } else {
        clauses.return_type
    };
    let MachinePath {
        name,
        attached_data,
        entry_name,
    } = split_machine_path(syntax_trees, path);

    let body_start = input;
    let (body, input) = parse_body(syntax_trees, input, machine_return_type.is_valid())?;
    if let Some(uses) = input.file_qualifiers {
        uses.close_declaration(
            declaration_start.source_span_until(input).span,
            name.source_span().span,
            body.as_ref()
                .map(|_| body_start.source_span_until(input).span),
        );
    }
    if body.is_some()
        && (!type_parameters.is_empty() || !conformance_bounds.is_empty())
        && let Some(uses) = input.file_qualifiers
    {
        uses.record_build_binder(
            syntax_trees,
            body_start.source_span_until(input).span,
            type_parameters,
            &conformance_bounds,
        );
    }
    let bodyless = body.is_none();
    let body = body.unwrap_or_default();
    let entry = State {
        name: entry_name.unwrap_or_else(|| Identifier::generated("entry")),
        parameters: machine_parameters,
        return_type: machine_return_type,
        contracts: HandleSpan::empty(),
        assembly_target_requirement: body.assembly_target_requirement,
        statements: body.statements,
    };
    let state = syntax_trees.items.insert_state(&entry);
    let state = syntax_trees.items.append_state_handle(state);
    if clauses.admitted_same_stack_bytes != 0 {
        let span = clauses
            .same_stack_clause_span
            .expect("a positive stack demand records its clause span");
        if !bodyless {
            return Err(crate::errors::parse_error::ParseError::at_source_span(
                "`stack N bytes` admits demand only on a bodyless external realization",
                span,
            ));
        }
        let has_external_binding = syntax_trees
            .items
            .satisfies_clauses(satisfies)
            .iter()
            .any(|clause| clause.via.is_some() || clause.via_expression.is_valid());
        if !has_external_binding {
            return Err(crate::errors::parse_error::ParseError::at_source_span(
                "`stack N bytes` requires a `satisfies ... via ...` binding",
                span,
            ));
        }
    }
    Ok((
        Machine {
            target_family_interface: false,
            name,
            generic_data_template: Default::default(),
            default_body_trait: Default::default(),
            attached_data,
            spelling,
            is_public: false,
            target: None,
            authored_target: None,
            boundary: false,
            bodyless,
            lifetime_parameters,
            type_parameters,
            satisfies,
            conformance_bounds,
            terminates_guarantee: clauses.terminates_guarantee,
            ranking_subjects: clauses.ranking_subjects,
            ranking_view: clauses.ranking_view,
            ranking_view_arguments: clauses.ranking_view_arguments,
            ranking_range: clauses.ranking_range,
            service_reach_keyword_source_spans: clauses.service_reach_keyword_source_spans,
            service_reach_is_installation_bound: clauses.service_reach_is_installation_bound,
            service_reaches: clauses.service_reaches,
            invokes: clauses.invokes,
            suspends_keyword_source_spans: clauses.suspends_keyword_source_spans,
            blocks_keyword_source_spans: clauses.blocks_keyword_source_spans,
            suspends: clauses.suspends,
            blocks: clauses.blocks,
            admitted_same_stack_bytes: clauses.admitted_same_stack_bytes,
            declares_no_crash_route: clauses.declares_no_crash_route,
            contracts: clauses.contracts,
            states: HandleSpan::from_parts(state, 1),
            nested_machines: body.nested_machines,
        },
        input,
    ))
}

struct MachinePath {
    name: Identifier,
    attached_data: Option<Identifier>,
    entry_name: Option<Identifier>,
}

fn split_machine_path(syntax_trees: &SyntaxTrees, path: HandleSpan<Identifier>) -> MachinePath {
    let members = syntax_trees.expressions.identifier_path_members(path);

    if members.len() <= 1 {
        return MachinePath {
            name: members
                .first()
                .cloned()
                .expect("machine path should contain a name"),
            attached_data: None,
            entry_name: None,
        };
    }

    let name = Identifier::from_path(syntax_trees, members);
    let attached_data = Identifier::from_path(syntax_trees, &members[..members.len() - 1]);

    MachinePath {
        name,
        attached_data: Some(attached_data),
        entry_name: members.last().cloned(),
    }
}
