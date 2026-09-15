use crate::parser::data::parse_machine_declaration_parameters;
use crate::parser::declaration::operator::parse_operator_spelling;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::state::{parse_optional_return_type, parse_optional_state_parameters};
use arena::HandleSpan;
use source::{SourceSpan, Span};
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::Machine;
use tokens::PunctuationKind;

pub(in crate::parser) mod body;
mod clauses;
mod parameter_contracts;

use body::rewrite_terminal_tail_self_calls;
pub(super) use clauses::parse_generic_conformance_bounds;
pub(in crate::parser) use clauses::parse_optional_satisfies_type_arguments;
pub(in crate::parser) use clauses::parse_satisfies_type_argument;
use clauses::{parse_machine_clauses, parse_satisfies_traits};
pub(super) use parameter_contracts::parse_machine_parameter_contracts;

pub(super) fn parse_machine<'tokens, 'source>(
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
    let (generic_parameters, input) = parse_machine_declaration_parameters(syntax_trees, input)?;
    let type_parameters = generic_parameters.type_parameters;
    let lifetime_parameters = generic_parameters.lifetime_parameters;
    let (machine_parameters, input) = parse_optional_state_parameters(syntax_trees, input)?;
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

    let ((states, bodyless), input) = body::parse_body(
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

pub(in crate::parser) fn join_path_identifier(members: &[Identifier]) -> Identifier {
    let mut name = String::new();

    for (index, member) in members.iter().enumerate() {
        if index > 0 {
            name.push_str("::");
        }

        name.push_str(member.as_str());
    }

    let first = members
        .first()
        .expect("joined machine path should contain a name")
        .source_span();
    let last = members
        .last()
        .expect("joined machine path should contain a name")
        .source_span();
    debug_assert_eq!(first.source_id, last.source_id);
    Identifier::new(
        name,
        SourceSpan::new(first.source_id, Span::new(first.span.start, last.span.end)),
    )
}

#[cfg(test)]
mod tests {
    use super::join_path_identifier;
    use source::{SourceId, SourceSpan, Span};
    use syntax_trees::identifier::Identifier;

    #[test]
    fn joined_machine_path_retains_authored_source_span() {
        let source = SourceId(7);
        let members = [
            Identifier::new("Provider", SourceSpan::new(source, Span::new(11, 19))),
            Identifier::new("first", SourceSpan::new(source, Span::new(21, 26))),
        ];

        let joined = join_path_identifier(&members);

        assert_eq!(joined.as_str(), "Provider::first");
        assert_eq!(
            joined.source_span(),
            SourceSpan::new(source, Span::new(11, 26))
        );
    }
}
