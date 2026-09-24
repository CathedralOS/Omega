use crate::bodies as body;
use crate::declarations::operator::parse_operator_spelling;
use crate::input::paths::join_path_identifier;
use crate::input::token_cursor::{Input, ParseResult, parse_path_handle_span};
use crate::parameters::parse_generic_parameters::GenericParameterSyntax;
use crate::parameters::parse_generic_parameters::parse_generic_parameters;
use crate::parameters::parse_parameters::{parse_optional_parameters, parse_optional_return_type};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::Machine;
use tokens::PunctuationKind;

use crate::bodies::tail_calls::rewrite_terminal_tail_self_calls;
use crate::contracts::conformance::parse_conformance::parse_satisfies_traits;
use crate::contracts::parse_contract_clauses::parse_machine_clauses;
use crate::parameters::contracts::parse_machine_parameter_contracts;

pub(crate) fn parse_machine<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Machine> {
    // OPERATOR-MACHINE-SUPPLY (spec expressions.md#executable-supply): an
    // optional fixed token immediately after `machine` binds operator syntax
    // to this ordinary named declaration (`machine + Vec2::add(...) -> Vec2`).
    // The vocabulary is the same closed `OperatorSpelling` set the `operator`
    // head uses; a non-punctuation head is a tokenless named machine. Every
    // `machine`-headed form shares this admission: plain, `boundary machine`,
    // and `<target> machine`. Item-level contexts that may not carry a token
    // (`boundary requirement`, conformance members) reject on the recorded
    // field after this returns; context ownership checks remain downstream.
    let (spelling, input) = if input.tokens.first().is_some_and(|token| {
        token.punctuation().is_some() && token.punctuation() != Some(PunctuationKind::Semicolon)
    }) {
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
    // A `satisfies` clause makes this machine a realization of a requirement
    // that owns the token binding; conformances and boundary realizations
    // supply implementations, never new token bindings (spec: a realizing
    // boundary machine uses `satisfies` "without redeclaring the token").
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
    let (
        (
            terminates_guarantee,
            ranking_subjects,
            ranking_view,
            ranking_view_arguments,
            ranking_range,
            service_reach_keyword_source_spans,
            service_reach_is_installation_bound,
            service_reaches,
            invokes,
            suspends_keyword_source_spans,
            blocks_keyword_source_spans,
            suspends,
            blocks,
            contracts,
            clauses_return_type,
            mut conformance_bounds,
            where_facts,
        ),
        next,
    ) = parse_machine_clauses(syntax_trees, next)?;
    let mut header_conformance_bounds = generic_parameters.conformance_bounds;
    header_conformance_bounds.append(&mut conformance_bounds);
    let conformance_bounds = header_conformance_bounds;
    input = next;
    // `-> T` is written either before the clauses or after them
    // (`terminates by ..; -> usize`); both spell the machine's return type.
    let machine_return_type = if machine_return_type.is_valid() {
        machine_return_type
    } else {
        clauses_return_type
    };
    let MachinePath {
        name,
        attached_data,
        entry_name,
    } = split_machine_path(syntax_trees, path);

    let ((states, bodyless), input) = body::parse_body::parse_body(
        syntax_trees,
        input,
        entry_name.clone(),
        machine_parameters,
        machine_return_type,
    )?;
    // Measured recursion MR2: a MEASURED machine's state whose TERMINAL
    // expression is a self-call to the machine's own entry (`{ self.sum(n -
    // 1, acc + n) }`, or the bare `sum(..)` in a free machine) is TAIL
    // recursion -- rewrite it to the loop-back transition `{ _ ->
    // <entry>(args) }` here, so every downstream pass (termination decrease
    // proof, loop-carried arg staging, both engines) sees the same bare
    // back-edge the arm spelling produces. Unmeasured machines keep the
    // call; validation names the missing measure.
    if !bodyless && !ranking_subjects.is_empty() {
        let entry_callable = entry_name.clone().unwrap_or_else(|| name.clone());
        rewrite_terminal_tail_self_calls(
            syntax_trees,
            states,
            &entry_callable,
            entry_name.is_some(),
        );
    }
    Ok((
        Machine {
            name,
            generic_data_template: Default::default(),
            attached_data,
            spelling,
            is_public: false,
            target: None,
            boundary: false,
            is_top_level_boundary_requirement: false,
            bodyless,
            lifetime_parameters,
            type_parameters,
            satisfies,
            conformance_bounds,
            where_facts,
            terminates_guarantee,
            ranking_subjects,
            ranking_view,
            ranking_view_arguments,
            ranking_range,
            service_reach_keyword_source_spans,
            service_reach_is_installation_bound,
            service_reaches,
            invokes,
            suspends_keyword_source_spans,
            blocks_keyword_source_spans,
            suspends,
            blocks,
            contracts,
            states,
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

    let name = join_path_identifier(members);
    let attached_data = join_path_identifier(&members[..members.len() - 1]);

    MachinePath {
        name,
        attached_data: Some(attached_data),
        entry_name: members.last().cloned(),
    }
}
